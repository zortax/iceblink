//! Glyph runs from a real font drawn by iceblink-core and the CPU renderer stay inside their
//! ink, at whole and fractional scales and at fractional positions.

mod common;

use iceblink::damage::DamageSet;
use iceblink::kurbo::BezPath;
use iceblink::{
    ArenaRange, ClipRef, Color, CompositorEvent, DeviceRect, FontData, FontId, Glyph, GlyphFlags,
    GlyphRun, Matrix4, OutlineKey, Paint, PaintKind, PenPosition, SpaceRef,
};
use iceblink::{Compositor as _, GlyphRaster as _};
use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_testing::golden::RgbaImage;
use iceblink_text::SwashRaster;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

use crate::common::content::{SIDE, Stage, append, background, key};
use crate::common::targets::on_cpu;

/// The scales the runs are drawn at, in device pixels per local unit.
const SCALES: [f32; 3] = [1.0, 1.5, 2.0];

/// Letters with ascenders, descenders, an accent and round tops and bottoms.
const TEXT: &str = "Ïnk, jog Qy";

/// The em size, in local units.
const SIZE: f32 = 11.0;

/// The pen's first position, between pixels on both axes at every scale.
const ORIGIN: [f32; 2] = [3.37, 20.41];

/// A run drawn on a white output in a frame of its own.
struct Drawn {
    /// The output before the run was recorded.
    before: RgbaImage,
    /// The output with the run.
    after: RgbaImage,
    /// The frame's draw damage, the ink of the new chunk.
    damage: DamageSet,
    /// Whether verification reported that the declared damage missed part of the derived damage.
    undeclared: bool,
}

#[test]
fn an_unhinted_run_draws_inside_the_ink_its_outlines_give() {
    for scale in SCALES {
        for (flags, embolden) in [
            (GlyphFlags::default(), 0.0),
            (GlyphFlags::SUBPIXEL, 0.0),
            (GlyphFlags::default(), 0.75),
            (GlyphFlags::SUBPIXEL, 0.75),
        ] {
            let what = format!("at scale {scale} with {flags:?} and emboldening {embolden}");
            let ink = outline_ink(scale, flags.contains(GlyphFlags::SUBPIXEL), embolden);
            let drawn = draw(scale, flags, embolden, Some(ink));

            assert_inside(&drawn, &what);
            assert_eq!(
                drawn.damage.rects(),
                &[ink],
                "{what}: the run's ink is the union of its outline boxes"
            );
            assert!(
                !drawn.undeclared,
                "{what}: a declaration of the outline boxes holds the derived damage"
            );
        }
    }
}

#[test]
fn hinted_runs_draw_inside_their_ink() {
    for scale in SCALES {
        for flags in [GlyphFlags::HINT, GlyphFlags::HINT | GlyphFlags::SUBPIXEL] {
            let drawn = draw(scale, flags, 0.0, None);

            assert_inside(&drawn, &format!("at scale {scale} with {flags:?}"));
        }
    }
}

/// Draws a white output, then records the run in its own chunk, declaring `declared` when given,
/// and draws again.
fn draw(scale: f32, flags: GlyphFlags, embolden: f32, declared: Option<DeviceRect>) -> Drawn {
    let mut target = on_cpu([SIDE, SIDE]).expect("the CPU renderer opens");
    let caps = target.renderer().capabilities();
    assert!(caps.subpixel_text, "the CPU renderer draws subpixel text");
    let mut stage = Stage::new(target, caps);
    stage.compositor.set_verification(true);
    let font = stage.compositor.font_registry().register(read_font(LATIN));
    stage.commit(|tx, ids| {
        tx.set_transform(ids.space, Matrix4::scale(scale, scale, 1.0));
        background(tx, ids);
    });
    let before = stage.draw();

    let output = stage.output();
    stage.commit(|tx, ids| {
        let black = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));
        let mut chunk = tx.record(key(2), ids.props());
        let paint = chunk.paint_ref(PaintKind::SOLID, black);
        let glyphs = chunk.glyphs(&line());
        chunk.glyph_run(GlyphRun {
            font,
            size: SIZE,
            glyphs,
            coords: ArenaRange::default(),
            paint,
            embolden,
            skew: 0.0,
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            flags,
            _pad: [0; 1],
            palette: 0,
        });
        append(tx, ids, 1, &[key(2)]);
        if let Some(ink) = declared {
            let mut damage = DamageSet::new();
            damage.absorb(ink);
            tx.declare_damage(output, damage);
        }
    });
    let after = stage.draw();

    let mut events = Vec::new();
    stage.compositor.drain_events(&mut events);
    Drawn {
        before,
        after,
        damage: *stage.damage(),
        undeclared: events
            .iter()
            .any(|event| matches!(event, CompositorEvent::DamageUndeclared { .. })),
    }
}

/// Asserts that every pixel the run changed lies inside the frame's damage.
#[track_caller]
fn assert_inside(drawn: &Drawn, what: &str) {
    let mut changed = 0;
    for y in 0..drawn.after.height() {
        for x in 0..drawn.after.width() {
            if drawn.before.pixel(x, y) == drawn.after.pixel(x, y) {
                continue;
            }
            changed += 1;
            let (x, y) = (x as i32, y as i32);
            let inside = drawn.damage.rects().iter().any(|rect| {
                let [x0, y0] = [rect.origin.x, rect.origin.y];
                (x0..x0 + rect.size.width).contains(&x) && (y0..y0 + rect.size.height).contains(&y)
            });
            assert!(
                inside,
                "{what}: pixel ({x}, {y}) changed outside the damage {:?}",
                drawn.damage
            );
        }
    }
    assert!(changed > 0, "{what}: the run draws");
}

/// Returns the union of the bitmap boxes the run's outlines give at `scale`, grown by
/// `embolden`.
fn outline_ink(scale: f32, subpixel: bool, embolden: f32) -> DeviceRect {
    let font = read_font(LATIN);
    let raster = SwashRaster::new();
    line()
        .iter()
        .filter_map(|glyph| {
            let [x0, y0, x1, y1] = outline_bounds(&raster, &font, glyph.id, SIZE * scale)?;
            let bounds = [x0 - embolden, y0 - embolden, x1 + embolden, y1 + embolden];
            PenPosition::of(scale * glyph.x).bitmap_box(scale * glyph.y, bounds, subpixel)
        })
        .reduce(DeviceRect::union)
        .expect("the text has outlines")
}

/// Returns the box of a glyph's outline points at the device size `size`, or `None` for a blank
/// glyph.
fn outline_bounds(
    raster: &SwashRaster,
    font: &FontData,
    glyph: u32,
    size: f32,
) -> Option<[f32; 4]> {
    let key = OutlineKey {
        font: FontId(0),
        glyph,
        size_bits: size.to_bits(),
        instance: 0,
        skew_bits: 0.0f32.to_bits(),
    };
    let mut path = BezPath::new();
    let drawn = raster.outline(font, &key, &[], &mut path);
    assert!(drawn, "the font has an outline for glyph {glyph}");
    if path.elements().is_empty() {
        return None;
    }
    let bounds = path.control_box();
    Some([
        bounds.x0 as f32,
        bounds.y0 as f32,
        bounds.x1 as f32,
        bounds.y1 as f32,
    ])
}

/// Returns the glyphs of [`TEXT`] at [`SIZE`] from a pen at [`ORIGIN`].
fn line() -> Vec<Glyph> {
    let font = read_font(LATIN);
    let face = skrifa::FontRef::new(font.bytes()).expect("the font parses");
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(Size::new(SIZE), LocationRef::default());
    let mut x = ORIGIN[0];
    TEXT.chars()
        .map(|character| {
            let id = charmap.map(character).unwrap_or_default();
            let glyph = Glyph {
                id: id.to_u32(),
                x,
                y: ORIGIN[1],
            };
            x += metrics.advance_width(id).unwrap_or(SIZE / 2.0);
            glyph
        })
        .collect()
}
