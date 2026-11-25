//! Glyph runs on the atlas route and the path route.

use iceblink::GlyphRaster;
use iceblink::damage::DamageSet;
use iceblink::kurbo::BezPath;
use iceblink::{
    ArenaRange, Built, ChunkRecorder, ClipRef, Color, CompositorEvent, DeviceRect, FontId,
    FrameClock, Glyph, GlyphFlags, GlyphRun, Matrix4, OrderEntry, OutlineKey, Paint, PaintKind,
    PaintRef, PenPosition, SpaceRef, Transaction, TransformId,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props};
use crate::conformance::scene::stage::{Stage, area, covers, presented};
use crate::fakes::{FakeGlyphRaster, fake_font};

/// A run of a solid paint under a translation draws one glyph mask per glyph, on whole device
/// pixels where the rasteriser placed its bitmap.
pub fn atlas_glyphs(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let font = stage.compositor.font_registry().register(fake_font(1));
    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    let glyphs = chunk.glyphs(&[
        Glyph {
            id: 65,
            x: 10.0,
            y: 40.0,
        },
        Glyph {
            id: 66,
            x: 30.25,
            y: 40.0,
        },
    ]);
    chunk.glyph_run(GlyphRun {
        font,
        size: 16.0,
        glyphs,
        coords: Default::default(),
        paint: PaintRef::color([0, 0, 0, 255]),
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::default(),
        _pad: [0],
        palette: 0,
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    let picture = stage.redraw();

    let (width, height) = FakeGlyphRaster::box_size(16.0);
    assert_eq!(picture.sprites.len(), 2, "one sprite per glyph");
    let first = &picture.sprites[0];
    assert_eq!(
        first.bounds,
        [10.0, 40.0 - height as f32, width as f32, height as f32],
        "a glyph at phase zero lands where its bitmap's origin is"
    );
    let second = &picture.sprites[1];
    assert_eq!(
        [second.bounds[0], second.bounds[1]],
        [30.0, 40.0 - height as f32],
        "a glyph a quarter pixel right lands on its pen pixel"
    );
}

/// A uniform positive scale is recognised within a relative 10⁻⁶, and on the path route every
/// glyph is a path of its own, emboldened by a stroke of `2 × embolden` device pixels.
pub fn glyph_routes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let font = stage.compositor.font_registry().register(fake_font(1));
    let mut tx = stage.tx();
    let mut node = identity(Some(stage.space));
    node.local = Matrix4::scale(2.0, 2.000_000_5, 1.0);
    let scaled = tx.create_transform(node);
    let sides = tx.create_paint(Paint::Sides([Color([0.0, 0.0, 0.0, 1.0]); 4]));
    let mut chunk = tx.record(key(1), props(scaled));
    let atlas = two_glyphs(&mut chunk);
    chunk.glyph_run(run(font, atlas, PaintRef::color([0, 0, 0, 255]), 0.0));
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    let picture = stage.redraw();
    assert_eq!(
        picture.sprites.len(),
        2,
        "a run under a scale of 2 and 2.0000005 takes the atlas route"
    );
    assert!(
        stage.frame.paths.is_empty(),
        "the atlas route draws no paths"
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), props(scaled));
    let glyphs = two_glyphs(&mut chunk);
    let paint = chunk.paint_ref(PaintKind::SIDES, sides);
    chunk.glyph_run(run(font, glyphs, paint, 1.0));
    stage.commit(tx);
    stage.redraw();
    let paths = &stage.frame.paths;
    assert_eq!(paths.len(), 2, "the path route draws one path per glyph");
    for path in paths {
        let width = path.stroke.as_ref().map(|stroke| stroke.style.width);
        let close = width.is_some_and(|width| (width - 1.0).abs() < 1.0e-3);
        assert!(
            close,
            "an embolden of one device pixel under a scale of 2 is a stroke one local unit wide, \
             got {width:?}"
        );
    }
}

/// A run under a slight scale that writes kept changing is rasterised at the device size and
/// placed on whole device pixels by the first frame presented 100 ms after the client's latest
/// frame before the last write.
pub fn glyphs_at_rest(make: MakeCompositor<'_>) {
    const MS: u64 = 1_000_000;
    let clock = |present_ns: u64| FrameClock {
        now_ns: present_ns,
        present_ns,
        interval_ns: 16 * MS,
    };
    let local = |scale: f32| Matrix4::scale(scale, scale, 1.0);

    let mut stage = Stage::full(make);
    let font = stage.compositor.font_registry().register(fake_font(1));
    let mut tx = stage.tx();
    let mut node = identity(Some(stage.space));
    node.local = local(0.8);
    let scaled = tx.create_transform(node);
    let mut chunk = tx.record(key(1), props(scaled));
    // At a scale of 0.9 the glyphs land at phase zero.
    let glyphs = chunk.glyphs(&[
        Glyph {
            id: 65,
            x: 10.0,
            y: 20.0,
        },
        Glyph {
            id: 66,
            x: 20.0,
            y: 20.0,
        },
    ]);
    chunk.glyph_run(run(font, glyphs, PaintRef::color([0, 0, 0, 255]), 0.0));
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.build_at(clock(0));
    stage.done(presented());
    for (step, scale) in [(1, 0.85), (2, 0.9)] {
        let mut tx = stage.tx();
        tx.set_transform(scaled, local(scale));
        stage.commit(tx);
        stage.build_at(clock(step * 16 * MS));
        stage.done(presented());
    }

    if stage.build_at(clock(116 * MS)) == Built::Frame {
        stage.done(presented());
    }
    let (width, height) = FakeGlyphRaster::box_size(8.0 * 0.9);
    let sprites = &stage.picture.sprites;
    assert_eq!(sprites.len(), 2, "the run takes the atlas route");
    for sprite in sprites {
        let [x, y, w, h] = sprite.bounds;
        assert!(
            x.fract() == 0.0 && y.fract() == 0.0 && [w, h] == [width as f32, height as f32],
            "a glyph at rest lands on a {width}x{height} box of whole device pixels, got {:?}",
            sprite.bounds
        );
    }
}

fn two_glyphs(chunk: &mut ChunkRecorder<'_>) -> ArenaRange {
    chunk.glyphs(&[
        Glyph {
            id: 65,
            x: 5.0,
            y: 20.0,
        },
        Glyph {
            id: 66,
            x: 15.0,
            y: 20.0,
        },
    ])
}

fn run(font: FontId, glyphs: ArenaRange, paint: PaintRef, embolden: f32) -> GlyphRun {
    GlyphRun {
        font,
        size: 8.0,
        glyphs,
        coords: Default::default(),
        paint,
        embolden,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::default(),
        _pad: [0],
        palette: 0,
    }
}

/// A glyph run's ink is the union of the boxes its bitmaps land on, which follow from the
/// outlines of unhinted glyphs without emboldening. Under scales of 1.5 and 2 at fractional
/// positions, a run recorded again in another colour damages exactly those boxes, and a declaration
/// of them holds the change. A declaration of the boxes before and after a fractional chunk offset
/// holds the move.
pub fn glyph_ink(make: MakeCompositor<'_>) {
    for scale in [1.5, 2.0] {
        glyph_ink_at(make, scale);
    }
}

/// Checks the ink of a run under a scale of `scale`, as [`glyph_ink`] describes.
fn glyph_ink_at(make: MakeCompositor<'_>, scale: f32) {
    const SIZE: f32 = 16.0;
    let glyphs = [
        Glyph {
            id: 65,
            x: 10.3,
            y: 20.6,
        },
        Glyph {
            id: 66,
            x: 21.45,
            y: 20.6,
        },
        Glyph {
            id: 67,
            x: 33.1,
            y: 31.17,
        },
    ];
    let mut stage = Stage::full(make);
    stage.compositor.set_verification(true);
    let font = stage.compositor.font_registry().register(fake_font(1));
    let mut tx = stage.tx();
    let mut node = identity(Some(stage.space));
    node.local = Matrix4::scale(scale, scale, 1.0);
    let scaled = tx.create_transform(node);
    record_run(&mut tx, scaled, font, &glyphs, SIZE, [0, 0, 0, 255]);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.draw();
    undeclared(&mut stage);

    let ink = |offset: [f32; 2]| {
        glyphs
            .iter()
            .filter_map(|glyph| {
                let bounds = fake_bounds(glyph.id, SIZE * scale);
                let [x, y] = [glyph.x + offset[0], glyph.y + offset[1]];
                PenPosition::of(scale * x).bitmap_box(scale * y, bounds, false)
            })
            .reduce(DeviceRect::union)
            .expect("the glyphs have boxes")
    };
    let before = ink([0.0; 2]);
    let mut tx = stage.tx();
    record_run(&mut tx, scaled, font, &glyphs, SIZE, [0, 0, 255, 255]);
    let mut declared = DamageSet::new();
    declared.absorb(before);
    tx.declare_damage(stage.output, declared);
    stage.commit(tx);
    stage.draw();
    assert!(
        covers(stage.damage(), before),
        "the damage {:?} holds the run's ink {before:?}",
        stage.damage()
    );
    let precise = stage.precise("glyph_ink");
    assert!(
        !precise || area(stage.damage()) == i64::from(before.size.width * before.size.height),
        "a precise compositor damages the run's ink {before:?} and nothing else, got {:?}",
        stage.damage()
    );
    assert!(
        !precise || !undeclared(&mut stage),
        "a declaration of the run's ink holds what the compositor derives"
    );

    const OFFSET: [f32; 2] = [0.37, 0.21];
    let after = ink(OFFSET);
    let mut tx = stage.tx();
    tx.set_offset(key(1), OFFSET);
    let mut declared = DamageSet::new();
    declared.absorb(before);
    declared.absorb(after);
    tx.declare_damage(stage.output, declared);
    stage.commit(tx);
    stage.draw();
    assert!(
        covers(stage.damage(), before) && covers(stage.damage(), after),
        "the damage {:?} holds the run's ink before and after the move, {before:?} and \
         {after:?}",
        stage.damage()
    );
    assert!(
        !precise || !undeclared(&mut stage),
        "a declaration of the run's ink before and after the move holds what the compositor \
         derives"
    );
}

/// Reports whether the compositor reported damage that a declaration missed.
fn undeclared(stage: &mut Stage) -> bool {
    let mut events = Vec::new();
    stage.compositor.drain_events(&mut events);
    events
        .iter()
        .any(|event| matches!(event, CompositorEvent::DamageUndeclared { .. }))
}

/// Records chunk 1 with one run of `glyphs` at `size` in `space`, painted `rgba`.
fn record_run(
    tx: &mut Transaction,
    space: TransformId,
    font: FontId,
    glyphs: &[Glyph],
    size: f32,
    rgba: [u8; 4],
) {
    let mut chunk = tx.record(key(1), props(space));
    let range = chunk.glyphs(glyphs);
    let mut run = run(font, range, PaintRef::color(rgba), 0.0);
    run.size = size;
    chunk.glyph_run(run);
}

/// Returns the box of the fake glyph's outline points at the device size `size`.
fn fake_bounds(glyph: u32, size: f32) -> [f32; 4] {
    let key = OutlineKey {
        font: FontId(0),
        glyph,
        size_bits: size.to_bits(),
        instance: 0,
        skew_bits: 0,
    };
    let mut path = BezPath::new();
    FakeGlyphRaster::new().outline(&fake_font(1), &key, &[], &mut path);
    let bounds = path.control_box();
    [
        bounds.x0 as f32,
        bounds.y0 as f32,
        bounds.x1 as f32,
        bounds.y1 as f32,
    ]
}
