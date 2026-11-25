//! Application shaders: shader quads in paint and coverage modes, filter shaders, and external
//! textures.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{plain_renderer, present};
use iceblink::Renderer as _;
use iceblink::{
    Backdrop, BlendMode, ClientId, ClipEntry, ClipLinkEntry, ClipRef, ExternalQuad, FRAME_NO_CLIP,
    FrameBackdrop, FrameClipShape, FrameExternal, FrameFilter, FrameGroup, LoweredChunk, PaintRef,
    ParamsBlock, Prebuilt, ROUND, RenderTarget, ShaderDesc, ShaderMode, ShaderOp, ShaderQuad,
    ShaderReads, ShaderSource, SpaceRef, TextureId,
};
use iceblink_wgpu::wgpu;

/// Returns a shader of `mode` from `wgsl`, with no parameters.
fn desc(mode: ShaderMode, wgsl: &str) -> Arc<ShaderDesc> {
    Arc::new(ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from(wgsl),
            prebuilt: None,
        },
        params_size: 0,
        reads: ShaderReads::default(),
    })
}

/// Returns a parameter block of zeros.
fn block() -> ParamsBlock {
    ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    }
}

/// Returns a shader quad of shader slot `shader` and parameter slot `params` over `bounds`,
/// filled with `fill`.
fn shader_quad(bounds: [f32; 4], shader: u32, params: u32, fill: PaintRef) -> ShaderQuad {
    ShaderQuad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        shader,
        params,
        opacity: 1.0,
    }
}

#[test]
fn a_paint_shader_shades_and_a_coverage_shader_shapes() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    builder.shader(ShaderOp::Create {
        shader: 0,
        desc: desc(
            ShaderMode::PAINT,
            "fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    return vec4<f32>(0.0, 0.0, 1.0, 1.0);\n}\n",
        ),
    });
    builder.shader(ShaderOp::Create {
        shader: 1,
        desc: desc(
            ShaderMode::COVERAGE,
            "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    return select(0.0, 1.0, input.uv.x < 0.5);\n}\n",
        ),
    });
    let params = builder.params(block());
    let green = builder.solid(opaque(0, 255, 0));
    builder.shader_quad(shader_quad(
        rect(0.0, 0.0, 64.0, 64.0),
        0,
        params,
        PaintRef::NONE,
    ));
    builder.shader_quad(shader_quad(rect(64.0, 0.0, 64.0, 64.0), 1, params, green));

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(32, 32),
        [0, 0, 255, 255],
        "the paint shader's colour"
    );
    assert_eq!(
        pixels.rgba(80, 32),
        [0, 255, 0, 255],
        "the fill inside the shape"
    );
    assert_eq!(pixels.rgba(120, 32), [0, 0, 0, 0], "and nothing outside it");
}

#[test]
fn a_filter_shader_replaces_a_group_s_content() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    builder.shader(ShaderOp::Create {
        shader: 0,
        desc: desc(
            ShaderMode::FILTER,
            "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    let texel = source_at(source, source_sampler, region, input.local);\n    return vec4<f32>(texel.b, texel.g, texel.r, texel.a);\n}\n",
        ),
    });
    let params = builder.params(block());
    let red = builder.solid(opaque(255, 0, 0));
    builder.begin_group(FrameGroup {
        bounds: rect(0.0, 0.0, 64.0, 64.0),
        source: rect(0.0, 0.0, 64.0, 64.0),
        clip: FRAME_NO_CLIP,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![FrameFilter::Shader {
            shader: 0,
            params,
            reach: 0.0,
        }]),
    });
    builder.quad(filled(rect(0.0, 0.0, 64.0, 64.0), red));
    builder.end_group();

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(32, 32),
        [0, 0, 255, 255],
        "red with its channels swapped"
    );
}

#[test]
fn an_external_quad_draws_its_bound_texture_and_nothing_unbound() {
    let mut renderer = plain_renderer();
    let device = renderer.gpu().device().clone();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    renderer.gpu().queue().write_texture(
        texture.as_image_copy(),
        &[0, 255, 0, 255].repeat(16),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(16),
            rows_per_image: Some(4),
        },
        texture.size(),
    );
    let client = ClientId(1);
    let ids = iceblink::IdSpace::new(client);
    let bound: TextureId = ids.mint();
    let unbound: TextureId = ids.mint();
    renderer.bind_external(client, bound, &texture, true);
    let mut builder = FrameBuilder::new();
    let quad = |x: f32, slot: u32, texture: TextureId| FrameExternal {
        quad: ExternalQuad {
            bounds: rect(x, 0.0, 32.0, 32.0),
            texture: slot,
            opacity: 1.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        client,
        texture,
    };
    builder.external(quad(0.0, 0, bound));
    builder.external(quad(64.0, 1, unbound));

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(16, 16), [0, 255, 0, 255]);
    assert_eq!(
        pixels.rgba(80, 16),
        [0, 0, 0, 0],
        "an unbound texture draws nothing"
    );
}

const RED: &str = "fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    return vec4<f32>(1.0, 0.0, 0.0, 1.0);\n}\n";

const LEFT_HALF: &str = "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    return select(0.0, 1.0, input.uv.x < 0.5);\n}\n";

/// Returns a frame that creates shader slot 0 of `mode` from `wgsl` and parameter slot 0.
fn with_shader(mode: ShaderMode, wgsl: &str) -> (FrameBuilder, u32) {
    let mut builder = FrameBuilder::new();
    builder.shader(ShaderOp::Create {
        shader: 0,
        desc: desc(mode, wgsl),
    });
    let params = builder.params(block());
    (builder, params)
}

#[test]
fn a_paint_shader_is_cut_by_its_corners_and_its_clip() {
    let mut renderer = plain_renderer();
    let (mut builder, params) = with_shader(ShaderMode::PAINT, RED);
    let mut rounded = shader_quad(rect(0.0, 0.0, 60.0, 60.0), 0, params, PaintRef::NONE);
    rounded.radii = [20.0; 8];
    builder.shader_quad(rounded);
    let clip = builder.clip(ClipEntry {
        links: [ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::RoundedRect {
                rect: rect(64.0, 0.0, 30.0, 30.0),
                radii: [0.0; 8],
                shapes: ROUND,
            },
        }]
        .into_iter()
        .collect(),
    });
    let mut clipped = shader_quad(rect(64.0, 0.0, 60.0, 60.0), 0, params, PaintRef::NONE);
    clipped.clip = clip;
    builder.shader_quad(clipped);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(30, 30),
        [255, 0, 0, 255],
        "the middle is shaded"
    );
    assert_eq!(pixels.rgba(1, 1)[3], 0, "the corner is cut");
    assert_eq!(
        pixels.rgba(80, 15),
        [255, 0, 0, 255],
        "inside the clip is shaded"
    );
    assert_eq!(pixels.rgba(110, 45)[3], 0, "outside it is not");
}

#[test]
fn a_filter_shader_samples_the_point_it_asks_for() {
    let mut renderer = plain_renderer();
    let (mut builder, params) = with_shader(
        ShaderMode::FILTER,
        "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    let texel = source_at(source, source_sampler, region, input.local - vec2<f32>(20.0, 0.0));\n    return vec4<f32>(texel.b, texel.g, texel.r, texel.a);\n}\n",
    );
    let red = builder.solid(opaque(255, 0, 0));
    builder.begin_group(FrameGroup {
        bounds: rect(0.0, 0.0, 60.0, 60.0),
        source: rect(0.0, 0.0, 60.0, 60.0),
        clip: FRAME_NO_CLIP,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![FrameFilter::Shader {
            shader: 0,
            params,
            reach: 0.0,
        }]),
    });
    builder.quad(filled(rect(0.0, 0.0, 20.0, 60.0), red));
    builder.end_group();

    let shifted = present(&mut renderer, &builder.finish()).rgba(30, 30);

    assert!(
        shifted[2] > 200,
        "twenty pixels right of the red, the shader reads red and swaps it to blue: {shifted:?}"
    );
}

#[test]
fn a_backdrop_shader_reads_the_composite_beneath_it() {
    let mut renderer = plain_renderer();
    let (mut builder, params) = with_shader(
        ShaderMode::FILTER,
        "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    let texel = source_at(source, source_sampler, region, input.local);\n    return vec4<f32>(texel.b, texel.g, texel.r, texel.a);\n}\n",
    );
    let red = builder.solid(opaque(255, 0, 0));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), red));
    let bounds = rect(0.0, 0.0, 64.0, 128.0);
    builder.backdrop(FrameBackdrop {
        backdrop: Backdrop {
            bounds,
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds,
        source: bounds,
        filters: Arc::from(vec![FrameFilter::Shader {
            shader: 0,
            params,
            reach: 0.0,
        }]),
    });

    let pixels = present(&mut renderer, &builder.finish());

    let filtered = pixels.rgba(32, 64);
    assert!(
        filtered[2] > 200 && filtered[0] < 40,
        "under the backdrop: {filtered:?}"
    );
    assert_eq!(
        pixels.rgba(96, 64),
        [255, 0, 0, 255],
        "beside it the red is untouched"
    );
}

/// Returns a quad of shader slot 0 over `bounds`, filled with red.
fn left_half(builder: &mut FrameBuilder, bounds: [f32; 4], params: u32) -> ShaderQuad {
    let red = builder.solid(opaque(255, 0, 0));
    shader_quad(bounds, 0, params, red)
}

// The shader is created once, in the first frame, and every later frame and configure keeps it.
#[test]
fn a_shader_draws_on_every_frame_and_after_every_configure() {
    let mut renderer = plain_renderer();
    for (frame, scale) in [1.0f32, 1.0, 2.0, 1.0, 1.5].into_iter().enumerate() {
        renderer.configure(RenderTarget {
            size: [128, 128],
            scale,
            opaque: true,
        });
        let mut builder = if frame == 0 {
            with_shader(ShaderMode::COVERAGE, LEFT_HALF).0
        } else {
            let mut builder = FrameBuilder::new();
            builder.params(block());
            builder
        };
        builder.frame_mut().scale = scale;
        let quad = left_half(&mut builder, rect(0.0, 0.0, 40.0 + frame as f32, 60.0), 0);
        builder.shader_quad(quad);
        let pixels = present(&mut renderer, &builder.finish());
        assert_eq!(pixels.rgba(5, 30)[3], 255, "frame {frame} at scale {scale}");
        assert_eq!(
            pixels.rgba(35, 30)[3],
            0,
            "frame {frame}: the right half is not covered"
        );
    }
}

// The chunk is inserted once and drawn in place, moved, and after a configure.
#[test]
fn a_resident_shader_quad_draws_in_place_moved_and_after_a_configure() {
    let mut renderer = plain_renderer();
    let (mut builder, params) = with_shader(ShaderMode::COVERAGE, LEFT_HALF);
    let quad = left_half(&mut builder, rect(0.0, 0.0, 40.0, 40.0), params);
    let chunk = Arc::new(LoweredChunk {
        shader_quads: vec![quad],
        ..LoweredChunk::default()
    });
    builder.insert(1, Arc::clone(&chunk));
    builder.draw_resident(1, [0.0, 0.0], &chunk);

    let first = present(&mut renderer, &builder.finish());

    assert_eq!(
        first.rgba(5, 20),
        [255, 0, 0, 255],
        "drawn on the frame it is inserted"
    );

    let mut again = FrameBuilder::next();
    again.draw_resident(1, [0.0, 0.0], &chunk);

    let replayed = present(&mut renderer, &again.finish());

    assert_eq!(
        replayed.max_difference(&first),
        0,
        "a replay draws the same picture"
    );

    let mut moved = FrameBuilder::next();
    moved.draw_resident(1, [20.0, 30.0], &chunk);

    let moved = present(&mut renderer, &moved.finish());

    assert_eq!(
        moved.rgba(25, 45),
        [255, 0, 0, 255],
        "inside the moved covered half"
    );
    assert_eq!(moved.rgba(5, 5)[3], 0, "nothing where the quad was");

    renderer.configure(RenderTarget {
        size: [128, 128],
        scale: 2.0,
        opaque: true,
    });
    let mut after = FrameBuilder::next();
    after.frame_mut().scale = 2.0;
    after.draw_resident(1, [0.0, 0.0], &chunk);

    let after = present(&mut renderer, &after.finish());

    assert_eq!(
        after.rgba(5, 20),
        [255, 0, 0, 255],
        "drawn after the configure"
    );
}

#[test]
fn a_resident_shader_lane_that_grows_keeps_the_quads_it_held() {
    let mut renderer = plain_renderer();
    let (mut builder, params) = with_shader(ShaderMode::COVERAGE, LEFT_HALF);
    let quad = left_half(&mut builder, rect(0.0, 0.0, 40.0, 20.0), params);
    let first = Arc::new(LoweredChunk {
        shader_quads: vec![quad; 2],
        ..LoweredChunk::default()
    });
    builder.insert(1, Arc::clone(&first));
    builder.draw_resident(1, [0.0, 0.0], &first);
    present(&mut renderer, &builder.finish());

    let quads: Vec<ShaderQuad> = (0..64)
        .map(|row| {
            let mut quad = quad;
            quad.bounds = rect(0.0, 40.0 + row as f32, 40.0, 1.0);
            quad
        })
        .collect();
    let grown = Arc::new(LoweredChunk {
        shader_quads: quads,
        ..LoweredChunk::default()
    });
    let mut next = FrameBuilder::next();
    next.insert(2, Arc::clone(&grown));
    next.draw_resident(1, [0.0, 0.0], &first);
    next.draw_resident(2, [0.0, 0.0], &grown);

    let pixels = present(&mut renderer, &next.finish());

    assert_eq!(
        pixels.rgba(5, 10),
        [255, 0, 0, 255],
        "the first chunk still draws"
    );
    assert_eq!(
        pixels.rgba(5, 70),
        [255, 0, 0, 255],
        "and so does the new one"
    );
}

const BLUE: &str = "fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    return vec4<f32>(0.0, 0.0, 1.0, 1.0);\n}\n";

const UNCOVERED: &str =
    "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    return 0.0;\n}\n";

const COVERED: &str =
    "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    return 1.0;\n}\n";

const UNCHANGED: &str = "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    return source_at(source, source_sampler, region, input.local);\n}\n";

const SWAPPED: &str = "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    return source_at(source, source_sampler, region, input.local).bgra;\n}\n";

/// Returns the colour a shader made from `desc` draws in the middle of a square.
///
/// A paint shader draws alone, a coverage shader is filled with green, and a filter shader
/// filters a red square.
fn drawn(desc: Arc<ShaderDesc>) -> [u8; 4] {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let mode = desc.mode;
    builder.shader(ShaderOp::Create { shader: 0, desc });
    let params = builder.params(block());
    let square = rect(0.0, 0.0, 64.0, 64.0);
    match mode {
        ShaderMode::PAINT => builder.shader_quad(shader_quad(square, 0, params, PaintRef::NONE)),
        ShaderMode::COVERAGE => {
            let green = builder.solid(opaque(0, 255, 0));
            builder.shader_quad(shader_quad(square, 0, params, green));
        }
        _ => {
            let red = builder.solid(opaque(255, 0, 0));
            builder.begin_group(FrameGroup {
                bounds: square,
                source: square,
                clip: FRAME_NO_CLIP,
                opacity: 1.0,
                blend: BlendMode::NORMAL,
                filters: Arc::from(vec![FrameFilter::Shader {
                    shader: 0,
                    params,
                    reach: 0.0,
                }]),
            });
            builder.quad(filled(square, red));
            builder.end_group();
        }
    }
    present(&mut renderer, &builder.finish()).rgba(32, 32)
}

/// Returns a shader of `mode` and `wgsl` whose prebuilt program, in `format`, is assembled from
/// `prebuilt` instead.
fn prebuilt(mode: ShaderMode, wgsl: &str, format: &str, prebuilt: &str) -> Arc<ShaderDesc> {
    let source = iceblink_wgsl::effect(mode, prebuilt).unwrap();
    let module = naga::front::wgsl::parse_str(&source).unwrap();
    let mut desc = Arc::unwrap_or_clone(desc(mode, wgsl));
    desc.source.prebuilt = Some(Prebuilt {
        format: Arc::from(format),
        bytes: Arc::from(bincode::serialize(&module).unwrap()),
    });
    Arc::new(desc)
}

#[test]
fn a_prebuilt_shader_of_the_renderer_s_format_draws_its_prebuilt_program() {
    let format = iceblink_wgsl::PREBUILT_FORMAT;
    let cases = [
        (ShaderMode::PAINT, BLUE, RED, [255, 0, 0, 255]),
        (ShaderMode::COVERAGE, UNCOVERED, COVERED, [0, 255, 0, 255]),
        (ShaderMode::FILTER, UNCHANGED, SWAPPED, [0, 0, 255, 255]),
    ];
    for (mode, wgsl, program, expected) in cases {
        assert_eq!(
            drawn(prebuilt(mode, wgsl, format, program)),
            expected,
            "{mode:?}"
        );
    }
}

#[test]
fn a_prebuilt_shader_of_another_format_draws_its_wgsl() {
    // A format name other than the renderer's.
    let desc = prebuilt(ShaderMode::PAINT, BLUE, "naga-30", RED);

    assert_eq!(drawn(desc), [0, 0, 255, 255]);
}

#[test]
fn a_snippet_may_declare_the_names_the_renderer_uses_inside() {
    let snippet = "const M_PI: f32 = 3.0;\n\
                   struct Bounds {\n    blue: f32,\n}\n\
                   fn clips(bounds: Bounds) -> f32 {\n    return bounds.blue / M_PI;\n}\n\
                   fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    \
                   return vec4<f32>(0.0, 0.0, clips(Bounds(3.0)), 1.0);\n}\n";
    assert_eq!(drawn(desc(ShaderMode::PAINT, snippet)), [0, 0, 255, 255]);
}
