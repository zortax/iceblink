//! Frames of the naive compositor drawn by the GPU renderer.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::color::{self, ColorSpace, GradientStop};
use iceblink::geom::{Point, Size};
use iceblink::kurbo::BezPath;
use iceblink::{
    ArenaRange, ChunkKey, ClipNode, ClipShape, DeviceRect, EffectNode, Extend, Glyph, GlyphFlags,
    GlyphRun, Gradient, GradientShape, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat,
    ImageWrite, Interpolation, MarkFlags, MarkPayload, MarkPlacement, Marks, OrderEntry, OrderNode,
    Paint, PaintKind, PaintRef, ROUND, SpaceRef,
};
use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_testing::gpu::Driver;
use iceblink_text::SwashRaster;
use iceblink_wgpu::Pixels;

use crate::common::naive::gpu::Rig;
use crate::common::naive::scene::{build, nudge};
use crate::common::naive::{Harness, quad};

const SIZE: [u32; 2] = [64, 48];

/// Asserts that every channel of the pixel at `at` is within `tolerance` of `expected`.
fn close(pixels: &Pixels, at: [i32; 2], expected: [u8; 4], tolerance: u8) {
    let actual = pixels.rgba(at[0], at[1]);
    let near = actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance);
    assert!(near, "pixel {at:?} is {actual:?}, expected {expected:?}");
}

/// Returns a tolerance of `steps`, one step wider on lavapipe.
fn tolerance(steps: u8) -> u8 {
    steps + u8::from(Driver::requested() == Driver::Lavapipe)
}

/// A harness whose root starts with an opaque white background.
fn white() -> Harness {
    let mut harness = Harness::with_size(SIZE);
    harness.add_quads(
        ChunkKey::new(0, 0),
        &[quad([0.0, 0.0, 64.0, 48.0], [255, 255, 255, 255])],
    );
    harness
}

#[test]
fn overlapping_quads_blend_in_paint_order() {
    let Some(mut rig) = Rig::open(SIZE) else {
        return;
    };
    let mut harness = white();
    harness.add_quads(
        ChunkKey::new(1, 0),
        &[
            quad([4.0, 4.0, 10.0, 10.0], [255, 0, 0, 255]),
            quad([8.0, 8.0, 10.0, 10.0], [0, 0, 128, 128]),
        ],
    );

    let pixels = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("a frame");
    close(&pixels, [5, 5], [255, 0, 0, 255], tolerance(1));
    close(&pixels, [12, 12], [128, 0, 128, 255], tolerance(2));
    close(&pixels, [16, 16], [127, 127, 255, 255], tolerance(2));
    close(&pixels, [40, 30], [255, 255, 255, 255], tolerance(0));
}

#[test]
fn a_group_is_composited_once_at_its_opacity() {
    let Some(mut rig) = Rig::open(SIZE) else {
        return;
    };
    let mut harness = white();
    let mut transaction = harness.transaction();
    let effect = transaction.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: Default::default(),
        filters: Arc::new([]),
        isolate: false,
    });
    let node = transaction.create_order(OrderNode {
        effect: Some(effect),
    });
    let key = ChunkKey::new(1, 0);
    let props = harness.props();
    let mut recorder = transaction.record(key, props);
    recorder.quad(quad([4.0, 4.0, 20.0, 20.0], [255, 0, 0, 255]));
    recorder.quad(quad([10.0, 10.0, 20.0, 20.0], [255, 0, 0, 255]));
    transaction.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
    harness.append(&mut transaction, OrderEntry::Node(node));
    harness.commit(transaction);

    let pixels = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("a frame");
    close(&pixels, [6, 6], [255, 128, 128, 255], tolerance(2));
    close(&pixels, [15, 15], [255, 128, 128, 255], tolerance(2));
}

#[test]
fn a_clip_node_cuts_what_is_drawn_through_it() {
    let Some(mut rig) = Rig::open(SIZE) else {
        return;
    };
    let mut harness = white();
    let mut transaction = harness.transaction();
    let space = harness.space;
    let clip = transaction.create_clip(ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [10.0, 10.0, 20.0, 20.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let key = ChunkKey::new(1, 0);
    let props = iceblink::ChunkProps {
        clip: Some(clip),
        ..harness.props()
    };
    transaction
        .record(key, props)
        .quad(quad([0.0, 0.0, 64.0, 48.0], [255, 0, 0, 255]));
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    let pixels = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("a frame");
    close(&pixels, [5, 5], [255, 255, 255, 255], tolerance(0));
    close(&pixels, [15, 15], [255, 0, 0, 255], tolerance(0));
    close(&pixels, [35, 15], [255, 255, 255, 255], tolerance(0));
}

#[test]
fn an_image_shows_its_texels() {
    let Some(mut rig) = Rig::open(SIZE) else {
        return;
    };
    let mut harness = white();
    let mut transaction = harness.transaction();
    let image = transaction.create_image(ImageDesc {
        width: 2,
        height: 2,
        format: ImageFormat::RGBA8,
    });
    let texels: Vec<u8> = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [0, 0, 0, 255],
    ]
    .concat();
    transaction.write_image(
        image,
        ImageWrite {
            rect: DeviceRect::new(Point::new(0, 0), Size::new(2, 2)),
            bytes: Arc::from(texels),
            offset: 0,
            stride: 8,
        },
    );
    let key = ChunkKey::new(1, 0);
    let props = harness.props();
    let mut recorder = transaction.record(key, props);
    let slot = recorder.image_slot(image);
    recorder.image(Image {
        bounds: [0.0, 0.0, 20.0, 20.0],
        frame: [0.0, 0.0, 20.0, 20.0],
        source: [0.0, 0.0, 2.0, 2.0],
        radii: [0.0; 8],
        shapes: ROUND,
        paint: PaintRef::NONE,
        image: slot,
        opacity: 1.0,
        clip: iceblink::ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::default(),
        extend: [Extend::NONE; 2],
        filter: ImageFilter::NEAREST,
    });
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    let pixels = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("a frame");
    close(&pixels, [3, 3], [255, 0, 0, 255], tolerance(1));
    close(&pixels, [16, 3], [0, 255, 0, 255], tolerance(1));
    close(&pixels, [3, 16], [0, 0, 255, 255], tolerance(1));
    close(&pixels, [16, 16], [0, 0, 0, 255], tolerance(1));
}

/// Records five Latin glyphs on the baseline `y` into chunk `key` with `paint`, and appends the
/// chunk to the root.
fn text(harness: &mut Harness, key: ChunkKey, paint: PaintRef, y: f32) {
    let font = harness
        .compositor
        .font_registry()
        .register(read_font(LATIN));
    let mut transaction = harness.transaction();
    let props = harness.props();
    let mut recorder = transaction.record(key, props);
    let glyphs: Vec<Glyph> = (0..5)
        .map(|index| Glyph {
            id: 40 + index,
            x: 2.0 + 11.0 * index as f32,
            y,
        })
        .collect();
    let range = recorder.glyphs(&glyphs);
    recorder.glyph_run(GlyphRun {
        font,
        size: 14.0,
        glyphs: range,
        coords: ArenaRange::default(),
        paint,
        embolden: 0.0,
        skew: 0.0,
        clip: iceblink::ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::default(),
        _pad: [0; 1],
        palette: 0,
    });
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);
}

/// Returns the box `[x0, y0, x1, y1]` of the pixels of rows `rows` that are not white, or `None`
/// when every pixel is white.
fn inked(pixels: &Pixels, rows: core::ops::Range<i32>) -> Option<[i32; 4]> {
    let mut inked = rows
        .flat_map(|y| (0..SIZE[0] as i32).map(move |x| (x, y)))
        .filter(|(x, y)| pixels.rgba(*x, *y)[1] < 224);
    let (x, y) = inked.next()?;
    Some(inked.fold([x, y, x, y], |[x0, y0, x1, y1], (x, y)| {
        [x0.min(x), y0.min(y), x1.max(x), y1.max(y)]
    }))
}

#[test]
fn glyphs_on_both_routes_ink_their_line_and_nothing_else() {
    let Some(mut rig) = Rig::open(SIZE) else {
        return;
    };
    let mut harness = white();
    harness
        .compositor
        .use_glyph_raster(Arc::new(SwashRaster::new()));
    text(
        &mut harness,
        ChunkKey::new(1, 0),
        PaintRef::color([0, 0, 0, 255]),
        16.0,
    );

    let mut transaction = harness.transaction();
    let black = color::Color::srgb(0.0, 0.0, 0.0, 1.0);
    let gradient = transaction.create_paint(Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [64.0, 0.0],
        },
        stops: Arc::new([GradientStop::new(0.0, black), GradientStop::new(1.0, black)]),
        interpolation: Interpolation::new(ColorSpace::Srgb),
        repeat: false,
    }));
    harness.commit(transaction);
    let paint = PaintRef {
        kind: PaintKind::GRADIENT,
        _pad: [0; 3],
        index: gradient.slot(),
    };
    text(&mut harness, ChunkKey::new(2, 0), paint, 40.0);

    let pixels = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("a frame");
    let atlas = inked(&pixels, 0..24).expect("the atlas route inks its line");
    let path = inked(&pixels, 24..48).expect("the path route inks its line");
    let moved = [atlas[0], atlas[1] + 24, atlas[2], atlas[3] + 24];
    let near = moved.iter().zip(path).all(|(a, b)| a.abs_diff(b) <= 1);
    assert!(
        near,
        "the atlas route inks {atlas:?} and the path route {path:?}"
    );
}

#[test]
fn every_kind_draws_and_an_edited_scene_equals_one_drawn_from_scratch() {
    let size = [96, 64];
    let Some(mut rig) = Rig::open(size) else {
        return;
    };
    let mut harness = Harness::with_size(size);
    build(&mut harness);
    rig.draw(&mut harness.compositor, harness.output)
        .expect("the scene draws");
    nudge(&mut harness);
    let incremental = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("the move draws");

    let Some(mut fresh_rig) = Rig::beside(size) else {
        return;
    };
    let mut fresh = Harness::with_size(size);
    build(&mut fresh);
    nudge(&mut fresh);
    let scratch = fresh_rig
        .draw(&mut fresh.compositor, fresh.output)
        .expect("the scene draws");

    assert_eq!(incremental.max_difference(&scratch), 0);
}

#[test]
fn a_mark_placement_lands_upright_at_its_quantised_anchor() {
    let Some(mut rig) = Rig::open(SIZE) else {
        return;
    };
    let mut harness = white();
    let mut triangle = BezPath::new();
    triangle.move_to((0.0, 0.0));
    triangle.line_to((6.0, 0.0));
    triangle.line_to((0.0, 6.0));
    triangle.close_path();

    let mut transaction = harness.transaction();
    let key = ChunkKey::new(1, 0);
    let props = harness.props();
    let mut recorder = transaction.record(key, props);
    let payload = recorder.payload(Arc::new(MarkPayload {
        shapes: vec![Arc::new(triangle)],
        placements: vec![MarkPlacement {
            x: 30.3,
            y: 12.7,
            shape: 0,
            _pad: 0,
        }],
        ..MarkPayload::default()
    }));
    recorder.marks(Marks {
        bounds: [0.0, 0.0, 64.0, 48.0],
        paint: PaintRef::color([0, 0, 0, 255]),
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 1.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements: 1,
        first: 0,
        payload,
        clip: iceblink::ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::default(),
    });
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    // The anchor quantises to (30.25, 12.75), and the right angle sits there, pointing up-left.
    let pixels = rig
        .draw(&mut harness.compositor, harness.output)
        .expect("a frame");
    for inked in [[31, 14], [33, 13], [31, 16]] {
        assert!(
            pixels.rgba(inked[0], inked[1])[1] < 96,
            "pixel {inked:?} is inked"
        );
    }
    for white in [[35, 17], [29, 14], [31, 11], [28, 14]] {
        close(&pixels, white, [255, 255, 255, 255], tolerance(0));
    }
}
