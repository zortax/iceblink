//! Sprites from atlas pages and external textures, drawn by both renderers, which must agree
//! within a tolerance.

mod common;

use std::sync::Arc;

use iceblink::Renderer as _;
use iceblink::{
    AtlasTile, ClientId, ClipRef, ColorSprite, ExternalQuad, FRAME_NO_CLIP, FrameExternal, IdSpace,
    PageFormat, ROUND, SpaceRef, Sprite, SpriteFlags, SubpixelOrder, TextureDesc,
};
use iceblink_cpu::ExternalImage;
use iceblink_testing::golden::Tolerance;
use iceblink_wgpu::wgpu;

use common::build::{FrameBuilder, filled};
use common::gpu::{assert_close, gpu};
use common::{cpu, draw, presented};

const CLOSE: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

// A 16 by 16 tile with a soft disc of coverage.
fn disc(channels: usize) -> Vec<u8> {
    let mut texels = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let distance = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            let coverage = ((7.0 - distance) * 64.0).clamp(0.0, 255.0) as u8;
            for channel in 0..channels {
                texels.push(if channel == 3 {
                    0
                } else {
                    coverage.saturating_sub(channel as u8 * 30)
                });
            }
        }
    }
    texels
}

// A 32 by 32 premultiplied checker of four colours.
fn checker() -> Vec<u8> {
    let mut texels = Vec::new();
    for y in 0..32u32 {
        for x in 0..32u32 {
            let colour: [u8; 4] = match ((x / 4) % 2, (y / 4) % 2) {
                (0, 0) => [200, 30, 30, 255],
                (1, 0) => [30, 160, 30, 255],
                (0, 1) => [20, 20, 100, 128],
                _ => [0, 0, 0, 0],
            };
            texels.extend_from_slice(&colour);
        }
    }
    texels
}

// The next level of a square premultiplied level of side `side`: each two by two block averaged.
fn halved(level: &[u8], side: u32) -> Vec<u8> {
    let half = side / 2;
    let at =
        |x: u32, y: u32, channel: u32| u32::from(level[((y * side + x) * 4 + channel) as usize]);
    let mut out = Vec::new();
    for y in 0..half {
        for x in 0..half {
            for channel in 0..4 {
                let sum = at(2 * x, 2 * y, channel)
                    + at(2 * x + 1, 2 * y, channel)
                    + at(2 * x, 2 * y + 1, channel)
                    + at(2 * x + 1, 2 * y + 1, channel);
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    out
}

fn sprite(bounds: [f32; 4], paint: iceblink::PaintRef, page: u32) -> Sprite {
    Sprite {
        bounds,
        paint,
        paint_origin: [0.0; 2],
        tile: AtlasTile {
            page,
            rect: [0, 0, 16, 16],
        },
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    }
}

fn color(
    bounds: [f32; 4],
    frame: [f32; 4],
    flags: SpriteFlags,
    paint: iceblink::PaintRef,
) -> ColorSprite {
    ColorSprite {
        bounds,
        frame,
        radii: [4.0; 8],
        shapes: ROUND,
        tile: AtlasTile {
            page: 3,
            rect: [0, 0, 32, 32],
        },
        paint,
        paint_origin: [0.0; 2],
        opacity: 0.9,
        clip: FRAME_NO_CLIP,
        space: 0,
        flags,
    }
}

#[test]
fn coverage_sprites_single_and_per_channel() {
    compare(
        "coverage sprites",
        &coverage_sprites(),
        SubpixelOrder::RedToBlue,
    );
}

#[test]
fn per_channel_sprites_on_a_display_whose_stripes_run_blue_to_red() {
    compare("blue to red", &coverage_sprites(), SubpixelOrder::BlueToRed);
}

fn coverage_sprites() -> FrameBuilder {
    let mut builder = FrameBuilder::new();
    let white = builder.solid([1.0; 4]);
    builder.quad(filled([0.0, 0.0, 96.0, 96.0], white));
    builder.page(1, PageFormat::A8, [16, 16], disc(1));
    let mut stripes = disc(4);
    for (index, texel) in stripes.iter_mut().enumerate() {
        if index % 4 == 3 {
            *texel = 0;
        }
    }
    builder.page(2, PageFormat::SUBPIXEL, [16, 16], stripes);
    let dark = builder.solid([0.1, 0.1, 0.3, 1.0]);
    let light = builder.solid([0.9, 0.8, 0.2, 1.0]);
    builder.mono(sprite([4.0, 4.0, 16.0, 16.0], dark, 1));
    builder.mono(sprite([24.0, 4.0, 16.0, 16.0], light, 1));
    // Resampled: two texels per pixel.
    builder.mono(sprite([44.0, 4.0, 32.0, 24.0], dark, 1));
    builder.subpixel(sprite([4.0, 40.0, 16.0, 16.0], dark, 2));
    builder.subpixel(sprite([24.0, 40.0, 16.0, 16.0], light, 2));
    builder
}

#[test]
fn colour_sprites_repeated_tinted_grey_and_minified() {
    let mut builder = FrameBuilder::new();
    let white = builder.solid([1.0; 4]);
    builder.quad(filled([0.0, 0.0, 96.0, 96.0], white));
    builder.page(3, PageFormat::RGBA8_PREMULTIPLIED, [32, 32], checker());
    let mut level = checker();
    for (index, side) in [16u32, 8, 4, 2, 1].into_iter().enumerate() {
        level = halved(&level, side * 2);
        builder.upload(
            3,
            index as u32 + 1,
            [side, side],
            level.clone().into(),
            side * 4,
        );
    }
    let tint = builder.solid([0.0, 0.4, 0.8, 0.8]);
    let none = iceblink::PaintRef::NONE;
    builder.color(color(
        [2.0, 2.0, 32.0, 32.0],
        [2.0, 2.0, 32.0, 32.0],
        SpriteFlags::default(),
        none,
    ));
    builder.color(color(
        [40.0, 2.0, 16.0, 16.0],
        [40.0, 2.0, 50.0, 40.0],
        SpriteFlags::REPEAT_X | SpriteFlags::REPEAT_Y,
        none,
    ));
    builder.color(color(
        [2.0, 48.0, 24.0, 24.0],
        [2.0, 48.0, 24.0, 24.0],
        SpriteFlags::GRAYSCALE,
        none,
    ));
    builder.color(color(
        [30.0, 48.0, 24.0, 24.0],
        [30.0, 48.0, 24.0, 24.0],
        SpriteFlags::MASK,
        tint,
    ));
    builder.color(color(
        [58.0, 48.0, 10.0, 10.0],
        [58.0, 48.0, 10.0, 10.0],
        SpriteFlags::default(),
        none,
    ));
    builder.color(color(
        [72.0, 48.0, 21.3, 21.3],
        [72.0, 48.0, 21.3, 21.3],
        SpriteFlags::NEAREST,
        none,
    ));
    compare("colour sprites", &builder, SubpixelOrder::RedToBlue);
}

fn compare(what: &str, builder: &FrameBuilder, order: SubpixelOrder) {
    let Some(mut gpu) = gpu() else {
        return;
    };
    let frame = builder.clone_frame();
    gpu.renderer.set_subpixel_order(order);
    let expected = gpu.drawn(&frame);
    let mut renderer = cpu();
    renderer.set_subpixel_order(order);
    draw(&mut renderer, &frame);
    assert_close(what, &expected, &presented(&mut renderer), CLOSE);
}

#[test]
fn an_external_texture_is_stretched_over_its_quad() {
    let Some(mut gpu) = gpu() else {
        return;
    };
    let ids = IdSpace::new(ClientId(1));
    let texture = ids.mint::<TextureDesc>();
    let rgba: Vec<u8> = (0..8u32 * 8)
        .flat_map(|index| [(index * 4) as u8, 255 - (index * 3) as u8, 90, 160])
        .collect();

    let device = gpu.renderer.gpu().clone();
    let held = device.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("external"),
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    device.queue().write_texture(
        held.as_image_copy(),
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(32),
            rows_per_image: Some(8),
        },
        wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
    );
    gpu.renderer
        .bind_external(ClientId(1), texture, &held, false);
    let mut renderer = cpu();
    renderer.bind_external(
        ClientId(1),
        texture,
        Arc::new(ExternalImage::new(8, 8, &rgba, false)),
    );

    let mut builder = FrameBuilder::new();
    let white = builder.solid([1.0; 4]);
    builder.quad(filled([0.0, 0.0, 96.0, 96.0], white));
    builder.external(FrameExternal {
        quad: ExternalQuad {
            bounds: [10.0, 12.0, 70.0, 50.0],
            texture: texture.slot(),
            opacity: 0.75,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        client: ClientId(1),
        texture,
    });
    let frame = builder.clone_frame();
    let expected = gpu.drawn(&frame);
    draw(&mut renderer, &frame);
    assert_close("external", &expected, &presented(&mut renderer), CLOSE);
}
