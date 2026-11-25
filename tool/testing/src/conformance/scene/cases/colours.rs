//! Colours in frames: item colours quantised to eight bits, and opacity judged on eight bits.

use std::sync::Arc;

use iceblink::{
    ClipRef, Color, EffectNode, Filter, FrameFilter, Glyph, GlyphFlags, GlyphRun, Line, LineStyle,
    OrderEntry, OrderNode, Paint, PaintKind, PaintRef, ROUND, RenderCapabilities, Shadow, SpaceRef,
    SubpixelOrder,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{faded, key, record_quads};
use crate::conformance::scene::stage::{FULL, Stage};
use crate::fakes::fake_font;

/// A premultiplied colour whose every component lies halfway between two eight-bit steps.
const HALVES: Color = Color([191.5 / 255.0, 64.5 / 255.0, 0.5 / 255.0, 254.5 / 255.0]);

/// A frame carries the colours of lines, shadows and drop-shadow filters quantised to eight bits,
/// as an inline colour packed from them paints.
pub fn item_colours(make: MakeCompositor<'_>) {
    let caps = RenderCapabilities {
        resident_chunks: false,
        ..FULL
    };
    let mut stage = Stage::new(make, caps);
    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    chunk.line(Line {
        bounds: [10.0, 10.0, 40.0, 2.0],
        color: HALVES,
        thickness: 2.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: LineStyle::SOLID,
        _pad: [0; 3],
    });
    chunk.shadow(Shadow {
        shape_bounds: [10.0, 30.0, 40.0, 20.0],
        radii: [0.0; 8],
        caster_bounds: [10.0, 30.0, 40.0, 20.0],
        caster_radii: [0.0; 8],
        shapes: ROUND,
        color: HALVES,
        blur: 2.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        inset: 0,
    });
    let shadowed = tx.create_effect(EffectNode {
        filters: Arc::from([Filter::DropShadow {
            offset: [2.0, 2.0],
            std_dev: 1.0,
            color: HALVES,
        }]),
        ..faded(1.0)
    });
    let node = tx.create_order(OrderNode {
        effect: Some(shadowed),
    });
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[([70.0, 70.0, 20.0, 20.0], [0, 0, 255, 255])],
    );
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(2))]);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.commit(tx);
    stage.redraw();

    let inline = Color::from_rgba8(HALVES.to_rgba8());
    let lines: Vec<Color> = stage
        .frame
        .lanes
        .lines
        .items
        .iter()
        .map(|line| line.color)
        .collect();
    assert_eq!(
        lines,
        [inline],
        "a line's colour is quantised to eight bits"
    );
    let shadows: Vec<Color> = stage
        .frame
        .lanes
        .shadows
        .items
        .iter()
        .map(|shadow| shadow.shadow.color)
        .collect();
    assert_eq!(
        shadows,
        [inline],
        "a shadow's colour is quantised to eight bits"
    );
    let filters: Vec<Color> = stage
        .frame
        .groups
        .iter()
        .flat_map(|group| group.filters.iter())
        .filter_map(|filter| match filter {
            FrameFilter::DropShadow { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(
        filters,
        [inline],
        "a drop-shadow filter's colour is quantised to eight bits"
    );
}

/// A solid slot whose alpha quantises to 255 is an opaque solid colour, as an inline alpha of 255
/// is, so a run painted with it is drawn as subpixel sprites.
pub fn opaque_alphas(make: MakeCompositor<'_>) {
    let caps = RenderCapabilities {
        subpixel_text: true,
        resident_chunks: false,
        ..FULL
    };
    let mut stage = Stage::new(make, caps);
    stage.subpixel = Some(SubpixelOrder::RedToBlue);
    let font = stage.compositor.font_registry().register(fake_font(1));
    let mut tx = stage.tx();
    let nearly = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 254.5 / 255.0])));
    let mut chunk = tx.record(key(1), stage.props());
    let slot = chunk.paint_ref(PaintKind::SOLID, nearly);
    for (paint, y) in [(slot, 30.0), (PaintRef::color([0, 0, 0, 255]), 60.0)] {
        let glyphs = chunk.glyphs(&[Glyph { id: 65, x: 10.0, y }]);
        chunk.glyph_run(GlyphRun {
            font,
            size: 16.0,
            glyphs,
            coords: Default::default(),
            paint,
            embolden: 0.0,
            skew: 0.0,
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            flags: GlyphFlags::SUBPIXEL,
            _pad: [0],
            palette: 0,
        });
    }
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    let picture = stage.redraw();

    let subpixel: Vec<bool> = picture
        .sprites
        .iter()
        .map(|sprite| sprite.subpixel)
        .collect();
    assert_eq!(
        subpixel,
        [true, true],
        "a slot alpha of 254.5 / 255 and an inline alpha of 255 both draw subpixel sprites"
    );
}
