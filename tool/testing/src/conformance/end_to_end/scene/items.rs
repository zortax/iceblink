//! The random colours, effects and chunks of a scene.

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Ellipse, Shape};
use iceblink::{
    BlendMode, BorderStyle, ChunkRecorder, ClipId, ClipRef, Color, EffectNode, FillRule, Filter,
    FontId, Glyph, GlyphFlags, GlyphRun, PaintKind, PaintRef, PathFill, PathItem, Quad, ROUND,
    SpaceRef, Transaction,
};

use crate::conformance::end_to_end::numbers::Numbers;
use crate::conformance::end_to_end::scene::{Ids, key, props};

/// Returns a random colour, opaque half of the time.
pub(super) fn colour(numbers: &mut Numbers) -> Color {
    let rgb = [(); 3].map(|_| numbers.below(256) as f32 / 255.0);
    let alpha = if numbers.below(2) == 0 {
        1.0
    } else {
        0.3 + numbers.below(7) as f32 * 0.1
    };
    Color([rgb[0] * alpha, rgb[1] * alpha, rgb[2] * alpha, alpha])
}

/// Returns a random isolating effect for order node `node`, which uses only the blend modes of
/// `blend_modes` and `NORMAL` in place of any other.
pub(super) fn effect(
    numbers: &mut Numbers,
    node: usize,
    port: ClipId,
    blend_modes: u32,
) -> EffectNode {
    let filters = match node {
        1 if numbers.below(3) == 0 => vec![Filter::DropShadow {
            offset: [numbers.below(9) as f32 - 4.0, numbers.below(9) as f32 - 4.0],
            std_dev: 1.0 + numbers.below(4) as f32,
            color: Color([0.0, 0.0, 0.0, 0.6]),
        }],
        1 | 4 => vec![Filter::Blur(0.5 + numbers.below(6) as f32 * 0.5)],
        _ => Vec::new(),
    };
    let blends = [
        BlendMode::NORMAL,
        BlendMode::MULTIPLY,
        BlendMode::SCREEN,
        BlendMode::DARKEN,
    ];
    let blend = match node {
        3 => blends[numbers.below(4) as usize],
        _ => BlendMode::NORMAL,
    };
    let blend = if blend_modes & (1 << blend.0) != 0 {
        blend
    } else {
        BlendMode::NORMAL
    };
    EffectNode {
        clip: (node == 3 && numbers.below(2) == 0).then_some(port),
        opacity: 0.4 + numbers.below(7) as f32 * 0.1,
        blend,
        filters: Arc::from(filters),
        isolate: true,
    }
}

/// Records chunk `part`: quads, a glyph run or a path, in the space its part chooses.
///
/// Chunks in the scrolled content are drawn through the port. The others lie mostly to the right
/// of it, and an eighth of their items anywhere on the output and past its edges.
pub(super) fn record(tx: &mut Transaction, ids: &Ids, numbers: &mut Numbers, part: u32) {
    let kind = (part % 3) as usize;
    let clip = (kind == 0).then_some(ids.port);
    let mut chunk = tx.record(key(part), props(ids.spaces[kind], clip));
    let anywhere = numbers.below(8) == 0;
    let (x, y) = match kind {
        0 => (numbers.quarter(4.0, 240), numbers.quarter(0.0, 1200)),
        _ if anywhere => (numbers.quarter(-16.0, 560), numbers.quarter(-16.0, 560)),
        _ => (numbers.quarter(72.0, 240), numbers.quarter(-16.0, 560)),
    };
    let size = [numbers.quarter(4.0, 120), numbers.quarter(3.0, 48)];

    match part % 4 {
        3 => glyph_run(&mut chunk, numbers, [x, y]),
        2 => ellipse(&mut chunk, ids, numbers, [x, y], size),
        _ => quads(&mut chunk, ids, numbers, [x, y], size),
    }
}

/// Records a run of one to four glyphs at `[x, y]`, in a random size and colour.
fn glyph_run(chunk: &mut ChunkRecorder<'_>, numbers: &mut Numbers, [x, y]: [f32; 2]) {
    let glyphs: Vec<Glyph> = (0..1 + numbers.below(4))
        .map(|index| Glyph {
            id: 65 + index as u32,
            x: x + index as f32 * 9.0,
            y: y + 14.0,
        })
        .collect();
    let range = chunk.glyphs(&glyphs);
    chunk.glyph_run(GlyphRun {
        font: FontId(0),
        size: 12.0 + numbers.below(8) as f32,
        glyphs: range,
        coords: Default::default(),
        paint: PaintRef::color([
            numbers.below(256) as u8 / 2,
            0,
            numbers.below(256) as u8 / 2,
            255,
        ]),
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::default(),
        _pad: [0],
        palette: 0,
    });
}

/// Records an ellipse path of `size` at `[x, y]`, filled with a random scene paint.
fn ellipse(
    chunk: &mut ChunkRecorder<'_>,
    ids: &Ids,
    numbers: &mut Numbers,
    [x, y]: [f32; 2],
    size: [f32; 2],
) {
    let paint = ids.paints[numbers.below(3) as usize];
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let ellipse = Ellipse::new(
        (f64::from(x + size[0] / 2.0), f64::from(y + size[1] / 2.0)),
        (f64::from(size[0] / 2.0), f64::from(size[1] / 2.0)),
        0.0,
    );
    let path: BezPath = ellipse.path_elements(0.1).collect();
    chunk.path(PathItem {
        path: Arc::new(path),
        placement: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint: fill,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
}

/// Records one or two quads of `size` near `[x, y]`, each filled with a random scene paint and
/// rounded or bordered at random.
fn quads(
    chunk: &mut ChunkRecorder<'_>,
    ids: &Ids,
    numbers: &mut Numbers,
    [x, y]: [f32; 2],
    size: [f32; 2],
) {
    for _ in 0..1 + numbers.below(2) {
        let paint = ids.paints[numbers.below(3) as usize];
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let radius = if numbers.below(3) == 0 {
            numbers.quarter(0.0, 24)
        } else {
            0.0
        };
        let border = if numbers.below(4) == 0 { 2.0 } else { 0.0 };
        chunk.quad(Quad {
            bounds: [
                x + numbers.below(8) as f32,
                y + numbers.below(8) as f32,
                size[0],
                size[1],
            ],
            radii: [radius; 8],
            shapes: ROUND,
            border: [border; 4],
            fill,
            stroke: PaintRef::color([20, 20, 20, 255]),
            paint_origin: [0.0; 2],
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            style: BorderStyle::SOLID,
            _pad: [0; 3],
        });
    }
}
