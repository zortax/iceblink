//! What glyph runs, images, marks, paths and groups lower to, and the pages they draw from.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::color::{self, ColorSpace, GradientStop};
use iceblink::geom::{Point, Size};
use iceblink::kurbo::{BezPath, Circle, Rect, Shape, Stroke};
use iceblink::{
    ArenaRange, BatchKind, ChunkKey, ClipRef, DeviceRect, EffectNode, Extend, FontId, FrameOutcome,
    Glyph, GlyphFlags, GlyphRun, Gradient, GradientShape, Image, ImageDesc, ImageFilter,
    ImageFlags, ImageFormat, ImageWrite, Interpolation, MarkFlags, MarkPayload, MarkPlacement,
    Marks, OrderEntry, OrderNode, PageFormat, PageOp, Paint, PaintKind, PaintRef, PathClip,
    PathFill, PathItem, PathStroke, ROUND, SpaceRef,
};
use iceblink_testing::fakes::{FakeGlyphRaster, fake_font};
use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_text::SwashRaster;

use crate::common::naive::{Harness, quad};

/// Gives the compositor a Swash glyph rasteriser and registers the Latin test font.
fn latin(harness: &mut Harness) -> FontId {
    harness
        .compositor
        .use_glyph_raster(Arc::new(SwashRaster::new()));
    harness
        .compositor
        .font_registry()
        .register(read_font(LATIN))
}

/// A solid run of `glyphs` at 16 pixels.
fn glyph_run(font: FontId, glyphs: ArenaRange, paint: PaintRef) -> GlyphRun {
    GlyphRun {
        font,
        size: 16.0,
        glyphs,
        coords: ArenaRange::default(),
        paint,
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::default(),
        _pad: [0; 1],
        palette: 0,
    }
}

/// Five glyphs on one baseline, at fractional positions.
fn glyphs() -> Vec<Glyph> {
    (0..5)
        .map(|index| Glyph {
            id: 40 + index,
            x: 4.3 + 9.0 * index as f32,
            y: 20.0,
        })
        .collect()
}

/// Returns how many pages of `format` `ops` creates.
fn created(ops: &[PageOp], format: PageFormat) -> usize {
    ops.iter()
        .filter(|op| matches!(op, PageOp::Create { format: created, .. } if *created == format))
        .count()
}

#[test]
fn a_solid_run_draws_mono_sprites_from_an_a8_page() {
    let mut harness = Harness::new();
    let font = latin(&mut harness);
    let mut transaction = harness.transaction();
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let range = recorder.glyphs(&glyphs());
    recorder.glyph_run(glyph_run(font, range, PaintRef::color([0, 0, 0, 255])));
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert_eq!(frame.lanes.mono.items.len(), 5);
    assert_eq!(created(&frame.resources.pages, PageFormat::A8), 1);
    assert!(
        frame
            .batches
            .iter()
            .all(|batch| matches!(batch.kind, BatchKind::Mono { .. }))
    );
    assert_eq!(frame.batches.len(), 1, "one run on one page is one batch");
}

#[test]
fn a_gradient_run_fills_each_glyph_as_a_path_in_a_pass_of_its_own() {
    let mut harness = Harness::new();
    let font = latin(&mut harness);
    let mut transaction = harness.transaction();
    let paint = transaction.create_paint(Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [40.0, 0.0],
        },
        stops: Arc::new([
            GradientStop::new(0.0, color::Color::srgb(1.0, 0.0, 0.0, 1.0)),
            GradientStop::new(1.0, color::Color::srgb(0.0, 0.0, 1.0, 1.0)),
        ]),
        interpolation: Interpolation::new(ColorSpace::Oklch),
        repeat: false,
    }));
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let range = recorder.glyphs(&glyphs());
    let paint = recorder.paint_ref(PaintKind::GRADIENT, paint);
    recorder.glyph_run(glyph_run(font, range, paint));
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert_eq!(frame.paths.len(), 5);
    assert_eq!(frame.vector_plan.passes.len(), 5);
    assert!(frame.lanes.mono.items.is_empty());
}

#[test]
fn an_image_draws_from_a_page_of_its_own_with_every_level() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let image = transaction.create_image(ImageDesc {
        width: 8,
        height: 4,
        format: ImageFormat::A8,
    });
    transaction.write_image(
        image,
        ImageWrite {
            rect: DeviceRect::new(Point::new(0, 0), Size::new(8, 4)),
            bytes: Arc::from(vec![200u8; 32]),
            offset: 0,
            stride: 8,
        },
    );
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let slot = recorder.image_slot(image);
    recorder.image(Image {
        bounds: [2.0, 2.0, 16.0, 8.0],
        frame: [2.0, 2.0, 16.0, 8.0],
        source: [0.0, 0.0, 8.0, 4.0],
        radii: [0.0; 8],
        shapes: ROUND,
        paint: PaintRef::NONE,
        image: slot,
        opacity: 1.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::default(),
        extend: [Extend::NONE; 2],
        filter: ImageFilter::LINEAR,
    });
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert_eq!(frame.lanes.color.items.len(), 1);
    let levels = frame.resources.pages.iter().find_map(|op| match op {
        PageOp::Create {
            format: PageFormat::RGBA8_PREMULTIPLIED,
            levels,
            ..
        } => Some(*levels),
        _ => None,
    });
    assert_eq!(levels, Some(4));
}

#[test]
fn union_marks_get_a_bin_and_placements_read_cells_from_an_a8_page() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let payload = recorder.payload(Arc::new(MarkPayload {
        discs: vec![[10.0, 10.0, 4.0, 0.0], [14.0, 10.0, 4.0, 0.0]],
        shapes: vec![Arc::new(Circle::new((0.0, 0.0), 3.0).to_path(0.1))],
        placements: vec![MarkPlacement {
            x: 30.0,
            y: 20.0,
            shape: 0,
            _pad: 0,
        }],
        ..MarkPayload::default()
    }));
    let marks = |discs, placements, flags| Marks {
        bounds: [0.0, 0.0, 64.0, 48.0],
        paint: PaintRef::color([0, 0, 0, 255]),
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 1.0,
        discs,
        boxes: 0,
        vertices: 0,
        placements,
        first: 0,
        payload,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
    };
    recorder.marks(marks(2, 0, MarkFlags::UNION));
    recorder.marks(marks(0, 1, MarkFlags::default()));
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert_eq!(frame.mark_plan.bins.len(), 1);
    assert_eq!(frame.mark_plan.bins[0].item, 0);
    assert_eq!(created(&frame.resources.pages, PageFormat::A8), 1);
}

#[test]
fn a_path_is_drawn_in_a_pass_of_its_own() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let mut outline = BezPath::new();
    outline.move_to((4.0, 4.0));
    outline.line_to((40.0, 8.0));
    outline.line_to((10.0, 30.0));
    outline.close_path();
    let item = PathItem {
        path: Arc::new(outline),
        placement: Default::default(),
        brush: Default::default(),
        fill: Some(PathFill {
            paint: PaintRef::color([0, 128, 0, 255]),
            rule: Default::default(),
        }),
        stroke: Some(PathStroke {
            paint: PaintRef::color([0, 0, 0, 255]),
            style: Stroke::new(2.0),
        }),
        // A path clip keeps the path a frame path.
        clips: Arc::new([PathClip {
            path: Arc::new(Rect::new(0.0, 0.0, 64.0, 64.0).to_path(0.1)),
            rule: Default::default(),
        }]),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    };
    recorder.path(item.clone());
    recorder.path(item);
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert_eq!(frame.vector_plan.passes.len(), 2);
    assert_eq!(frame.vector_plan.passes[0].region.origin, Point::new(0, 0));
}

#[test]
fn an_isolating_effect_draws_its_subtree_in_a_group_inside_the_output() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let effect = transaction.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: Default::default(),
        filters: Arc::new([iceblink::Filter::Blur(3.0)]),
        isolate: false,
    });
    let node = transaction.create_order(OrderNode {
        effect: Some(effect),
    });
    let key = ChunkKey::new(1, 0);
    let props = harness.props();
    transaction
        .record(key, props)
        .quad(quad([-4.0, 30.0, 20.0, 20.0], [255, 0, 0, 255]));
    transaction.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
    harness.append(&mut transaction, OrderEntry::Node(node));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert_eq!(frame.groups.len(), 1);
    let [x, y, width, height] = frame.groups[0].bounds;
    assert!(x >= 0.0 && y >= 0.0 && x + width <= 64.0 && y + height <= 48.0);
    let kinds: Vec<_> = frame.batches.iter().map(|batch| batch.kind).collect();
    assert_eq!(
        kinds,
        [
            BatchKind::GroupBegin(0),
            BatchKind::Quads,
            BatchKind::GroupEnd(0)
        ]
    );
}

#[test]
fn a_recovered_renderer_receives_every_page_again() {
    let mut harness = Harness::new();
    let font = latin(&mut harness);
    let mut transaction = harness.transaction();
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let range = recorder.glyphs(&glyphs());
    recorder.glyph_run(glyph_run(font, range, PaintRef::color([0, 0, 0, 255])));
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame_with(FrameOutcome::Recovered));
    assert!(harness.frame());
    assert_eq!(created(&harness.frame.resources.pages, PageFormat::A8), 1);

    harness.compositor.forget_caches();
    assert!(harness.frame());
    let destroyed = harness
        .frame
        .resources
        .pages
        .iter()
        .filter(|op| matches!(op, PageOp::Destroy { .. }))
        .count();
    assert_eq!(destroyed, 1);
    assert_eq!(created(&harness.frame.resources.pages, PageFormat::A8), 1);
}

#[test]
fn a_gradient_in_oklab_keeps_its_stops_in_that_encoding() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let paint = transaction.create_paint(Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [40.0, 0.0],
        },
        stops: Arc::new([
            GradientStop::new(0.0, color::Color::srgb(1.0, 0.0, 0.0, 1.0)),
            GradientStop::new(1.0, color::Color::srgb(0.0, 0.0, 1.0, 1.0)),
        ]),
        interpolation: Interpolation::new(ColorSpace::Oklab),
        repeat: false,
    }));
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let mut item = quad([0.0, 0.0, 40.0, 10.0], [0; 4]);
    item.fill = recorder.paint_ref(PaintKind::GRADIENT, paint);
    recorder.quad(item);
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    let gradient = frame
        .tables
        .paints
        .all
        .iter()
        .find_map(|paint| match paint {
            iceblink::FramePaint::Gradient(gradient) => Some(gradient),
            _ => None,
        });
    let gradient = gradient.expect("the gradient is in the paint table");
    assert_eq!(gradient.encoding, iceblink::StopEncoding::OKLAB);
    assert_eq!(gradient.stops.len(), 2);
}

#[test]
fn a_group_left_out_for_drawing_nothing_leaves_no_mark_bin() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let space = harness.space;
    let clip = transaction.create_clip(iceblink::ClipNode {
        parent: None,
        space,
        shape: iceblink::ClipShape::RoundedRect {
            rect: [100.0, 100.0, 10.0, 10.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let effect = transaction.create_effect(EffectNode {
        clip: Some(clip),
        opacity: 0.5,
        blend: Default::default(),
        filters: Arc::new([]),
        isolate: false,
    });
    let node = transaction.create_order(OrderNode {
        effect: Some(effect),
    });
    let marks = |payload, flags| Marks {
        bounds: [0.0, 0.0, 64.0, 48.0],
        paint: PaintRef::color([0, 0, 0, 255]),
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 1.0,
        discs: 1,
        boxes: 0,
        vertices: 0,
        placements: 0,
        first: 0,
        payload,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
    };
    let payload = Arc::new(MarkPayload {
        discs: vec![[10.0, 10.0, 4.0, 0.0]],
        ..MarkPayload::default()
    });

    let inside = ChunkKey::new(1, 0);
    let props = harness.props();
    let mut recorder = transaction.record(inside, props);
    let index = recorder.payload(payload.clone());
    recorder.marks(marks(index, MarkFlags::UNION));
    transaction.splice(node, 0, 0, &[OrderEntry::Chunk(inside)]);
    harness.append(&mut transaction, OrderEntry::Node(node));

    let outside = ChunkKey::new(2, 0);
    let mut recorder = transaction.record(outside, props);
    let index = recorder.payload(payload);
    recorder.marks(marks(index, MarkFlags::default()));
    harness.append(&mut transaction, OrderEntry::Chunk(outside));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert!(frame.groups.is_empty());
    assert_eq!(frame.lanes.marks.items.len(), 1);
    assert!(frame.mark_plan.is_empty());
}

#[test]
fn a_new_glyph_rasteriser_rasterises_again_even_after_every_chunk_was_removed() {
    let mut harness = Harness::new();
    let font = harness.compositor.font_registry().register(fake_font(1));
    let first = Arc::new(FakeGlyphRaster::new());
    harness.compositor.use_glyph_raster(first.clone());

    let key = ChunkKey::new(1, 0);
    let record = |harness: &mut Harness| {
        let mut transaction = harness.transaction();
        let props = harness.props();
        let mut recorder = transaction.record(key, props);
        let range = recorder.glyphs(&glyphs());
        recorder.glyph_run(glyph_run(font, range, PaintRef::color([0, 0, 0, 255])));
        transaction
    };
    let mut transaction = record(&mut harness);
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);
    assert!(harness.frame());
    assert!(first.rasterised() > 0);

    let mut transaction = harness.transaction();
    transaction.remove_chunk(key);
    harness.commit(transaction);
    assert!(harness.frame());

    let second = Arc::new(FakeGlyphRaster::new());
    harness.compositor.use_glyph_raster(second.clone());
    let transaction = record(&mut harness);
    harness.commit(transaction);
    assert!(harness.frame());
    assert!(second.rasterised() > 0);
}

#[test]
fn a_glyph_tile_larger_than_a_shared_page_draws_nothing_and_creates_no_page() {
    let mut harness = Harness::new();
    harness
        .compositor
        .use_glyph_raster(Arc::new(FakeGlyphRaster::new()));
    let font = harness.compositor.font_registry().register(fake_font(1));
    let mut transaction = harness.transaction();
    let props = harness.props();
    let key = ChunkKey::new(1, 0);
    let mut recorder = transaction.record(key, props);
    let range = recorder.glyphs(&[Glyph {
        id: FakeGlyphRaster::COLOR,
        x: 4.0,
        y: 60.0,
    }]);
    let mut run = glyph_run(font, range, PaintRef::color([0, 0, 0, 255]));
    run.size = 3000.0;
    recorder.glyph_run(run);
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert!(
        frame.lanes.color.items.is_empty(),
        "the glyph draws nothing"
    );
    assert!(
        !frame
            .resources
            .pages
            .iter()
            .any(|op| matches!(op, PageOp::Create { .. })),
        "no page is created for a tile that fits on none"
    );
}
