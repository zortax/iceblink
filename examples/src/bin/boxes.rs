//! Boxes with borders, gradients and shadows, labelled with text, and one card that slides back
//! and forth by writes to its transform.

use std::sync::Arc;

use iceblink::color::{Color as StopColor, ColorSpace};
use iceblink::{
    Anchor, BorderStyle, ChunkKey, ChunkRecorder, ClipRef, Color, Gradient, GradientShape,
    GradientStop, Interpolation, Matrix4, Paint, PaintId, PaintKind, PaintRef, ROUND, Shadow,
    SpaceRef, Transaction, TransformId, TransformNode,
};
use iceblink_examples::{Demo, Font, Line, Scene, quad, solid};

const BACKGROUND: ChunkKey = ChunkKey::new(1, 0);
const CARDS: ChunkKey = ChunkKey::new(2, 0);
const SLIDER: ChunkKey = ChunkKey::new(3, 0);

/// The size of a card.
const CARD: [f32; 2] = [160.0, 110.0];

/// The gap between cards.
const GAP: f32 = 24.0;

/// How far the sliding card travels to each side.
const TRAVEL: f32 = 180.0;

/// How long one swing of the sliding card takes, in seconds.
const PERIOD: f32 = 8.0;

/// Cards of boxes and a card that slides back and forth.
struct Boxes {
    background: PaintId,
    slider: TransformId,
}

impl Demo for Boxes {
    const TITLE: &'static str = "iceblink boxes";

    fn new(scene: &mut Scene, tx: &mut Transaction, size: [f32; 2]) -> Self {
        let background = solid(tx, [0.94, 0.95, 0.97, 1.0]);
        record_background(scene, tx, background, size);
        scene.append(tx, BACKGROUND);

        record_cards(scene, tx);
        scene.append(tx, CARDS);

        let slider = tx.create_transform(TransformNode {
            parent: Some(scene.space()),
            local: Matrix4::IDENTITY,
            scroll: None,
            anchor: Anchor::Scrolls,
        });
        record_slider(scene, tx, slider);
        scene.append(tx, SLIDER);
        Self { background, slider }
    }

    fn resize(&mut self, scene: &Scene, tx: &mut Transaction, size: [f32; 2]) {
        record_background(scene, tx, self.background, size);
    }

    fn advance(&mut self, tx: &mut Transaction, seconds: f32) -> bool {
        let phase = seconds / PERIOD * std::f32::consts::TAU;
        tx.set_transform(
            self.slider,
            Matrix4::translation(TRAVEL * phase.sin(), 0.0, 0.0),
        );
        true
    }
}

/// Records the background of a window of `size`.
fn record_background(scene: &Scene, tx: &mut Transaction, paint: PaintId, size: [f32; 2]) {
    let mut chunk = tx.record(BACKGROUND, Scene::props(scene.space()));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 0.0, size[0], size[1]], fill));
}

/// Returns a ramp through `colors`, spaced evenly, interpolated in Oklab.
fn ramp(shape: GradientShape, colors: &[[f32; 3]]) -> Paint {
    let last = (colors.len() - 1) as f32;
    let stops = colors
        .iter()
        .enumerate()
        .map(|(index, &rgb)| {
            GradientStop::new(
                index as f32 / last,
                StopColor::new(ColorSpace::Srgb, rgb, 1.0),
            )
        })
        .collect::<Vec<_>>();
    Paint::Gradient(Gradient {
        shape,
        stops: Arc::from(stops),
        interpolation: Interpolation::new(ColorSpace::Oklab),
        repeat: false,
    })
}

/// Returns the bounds of card `column` in row `row`.
fn card(column: usize, row: usize) -> [f32; 4] {
    [
        GAP + column as f32 * (CARD[0] + GAP),
        72.0 + row as f32 * (CARD[1] + 64.0),
        CARD[0],
        CARD[1],
    ]
}

/// Returns the shadow of a card at `bounds` with corner `radius`, cast `offset` down.
fn shadow(bounds: [f32; 4], radius: f32, blur: f32, offset: f32) -> Shadow {
    let [x, y, width, height] = bounds;
    Shadow {
        shape_bounds: [x, y + offset, width, height],
        radii: [radius; 8],
        caster_bounds: bounds,
        caster_radii: [radius; 8],
        shapes: ROUND,
        color: Color([0.05, 0.08, 0.2, 0.35]),
        blur,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        inset: 0,
    }
}

/// The paints of the cards.
struct CardPaints {
    white: PaintId,
    ink: PaintId,
    accent: PaintId,
    warm: PaintId,
    linear: PaintId,
    radial: PaintId,
    conic: PaintId,
}

/// The chunk's references to the solid paints of the cards.
struct CardRefs {
    text: PaintRef,
    white: PaintRef,
    accent: PaintRef,
    warm: PaintRef,
}

/// Records the title and two rows of cards: one row of fills, then one row of borders and
/// shadows.
fn record_cards(scene: &Scene, tx: &mut Transaction) {
    let paints = card_paints(tx);

    let font = scene.font();
    let mut chunk = tx.record(CARDS, Scene::props(scene.space()));
    let text = chunk.paint_ref(PaintKind::SOLID, paints.ink);
    font.record(
        &mut chunk,
        &Line {
            text: "Boxes, borders, gradients and shadows",
            size: 24.0,
            origin: [GAP, 44.0],
            paint: text,
        },
    );

    let refs = CardRefs {
        text,
        white: chunk.paint_ref(PaintKind::SOLID, paints.white),
        accent: chunk.paint_ref(PaintKind::SOLID, paints.accent),
        warm: chunk.paint_ref(PaintKind::SOLID, paints.warm),
    };
    let fills = [
        ("Rounded, bordered", refs.white),
        (
            "Linear ramp",
            chunk.paint_ref(PaintKind::GRADIENT, paints.linear),
        ),
        (
            "Radial ramp",
            chunk.paint_ref(PaintKind::GRADIENT, paints.radial),
        ),
        (
            "Conic ramp",
            chunk.paint_ref(PaintKind::GRADIENT, paints.conic),
        ),
    ];
    record_fills(&mut chunk, font, &refs, fills);
    record_borders(&mut chunk, font, &refs);
}

/// Creates the paints of the cards.
fn card_paints(tx: &mut Transaction) -> CardPaints {
    let white = solid(tx, [1.0, 1.0, 1.0, 1.0]);
    let ink = solid(tx, [0.1, 0.12, 0.18, 1.0]);
    let accent = solid(tx, [0.18, 0.38, 0.86, 1.0]);
    let warm = solid(tx, [0.92, 0.42, 0.25, 1.0]);
    let linear = tx.create_paint(ramp(
        GradientShape::Linear {
            start: [0.0, 0.0],
            end: [CARD[0], CARD[1]],
        },
        &[[0.49, 0.89, 1.0], [0.18, 0.42, 1.0], [0.82, 0.55, 1.0]],
    ));
    let radial = tx.create_paint(ramp(
        GradientShape::Radial {
            center: [CARD[0] / 2.0, CARD[1] / 2.0],
            radius: [CARD[0] / 2.0, CARD[1] / 2.0],
        },
        &[[1.0, 0.93, 0.55], [0.98, 0.45, 0.32], [0.45, 0.12, 0.36]],
    ));
    let conic = tx.create_paint(ramp(
        GradientShape::Conic {
            center: [CARD[0] / 2.0, CARD[1] / 2.0],
            angle: 0.0,
        },
        &[
            [0.95, 0.3, 0.3],
            [0.3, 0.85, 0.4],
            [0.3, 0.4, 0.95],
            [0.95, 0.3, 0.3],
        ],
    ));

    CardPaints {
        white,
        ink,
        accent,
        warm,
        linear,
        radial,
        conic,
    }
}

/// Records the first row: one rounded card for each of `fills`, the first one bordered.
fn record_fills(
    chunk: &mut ChunkRecorder<'_>,
    font: &Font,
    refs: &CardRefs,
    fills: [(&str, PaintRef); 4],
) {
    for (column, (label, fill)) in fills.into_iter().enumerate() {
        let bounds = card(column, 0);
        chunk.shadow(shadow(bounds, 14.0, 10.0, 6.0));
        let mut face = quad(bounds, fill);
        face.radii = [14.0; 8];
        if column == 0 {
            face.border = [3.0; 4];
            face.stroke = refs.accent;
        }
        chunk.quad(face);
        record_label(chunk, font, label, bounds, refs.text);
    }
}

/// Records the second row: white cards with border styles and shadows of different blurs.
fn record_borders(chunk: &mut ChunkRecorder<'_>, font: &Font, refs: &CardRefs) {
    let borders = [
        ("Dashed border", BorderStyle::DASHED, 10.0, 6.0_f32),
        ("Dotted border", BorderStyle::DOTTED, 10.0, 6.0),
        ("Square, hard shadow", BorderStyle::SOLID, 0.0, 1.0),
        ("Soft, wide shadow", BorderStyle::SOLID, 10.0, 28.0),
    ];
    for (column, (label, style, radius, blur)) in borders.into_iter().enumerate() {
        let bounds = card(column, 1);
        chunk.shadow(shadow(bounds, radius, blur, 8.0));
        let mut face = quad(bounds, refs.white);
        face.radii = [radius; 8];
        face.border = [4.0; 4];
        face.stroke = if column % 2 == 0 {
            refs.accent
        } else {
            refs.warm
        };
        face.style = style;
        chunk.quad(face);
        record_label(chunk, font, label, bounds, refs.text);
    }
}

/// Records `text` under the card at `bounds`.
fn record_label(
    chunk: &mut ChunkRecorder<'_>,
    font: &Font,
    text: &str,
    bounds: [f32; 4],
    paint: PaintRef,
) {
    font.record(
        chunk,
        &Line {
            text,
            size: 15.0,
            origin: [bounds[0] + 4.0, bounds[1] + bounds[3] + 24.0],
            paint,
        },
    );
}

/// Records the sliding card in `space`.
fn record_slider(scene: &Scene, tx: &mut Transaction, space: TransformId) {
    let paint = tx.create_paint(ramp(
        GradientShape::Linear {
            start: [0.0, 0.0],
            end: [240.0, 0.0],
        },
        &[[0.16, 0.7, 0.55], [0.15, 0.45, 0.75]],
    ));
    let ink = solid(tx, [1.0, 1.0, 1.0, 1.0]);
    let bounds = [280.0, 440.0, 240.0, 72.0];
    let mut chunk = tx.record(SLIDER, Scene::props(space));
    chunk.shadow(shadow(bounds, 36.0, 12.0, 6.0));
    let fill = chunk.paint_ref(PaintKind::GRADIENT, paint);
    let mut face = quad(bounds, fill);
    face.radii = [36.0; 8];
    chunk.quad(face);
    let text = chunk.paint_ref(PaintKind::SOLID, ink);
    scene.font().record(
        &mut chunk,
        &Line {
            text: "Moved by set_transform",
            size: 18.0,
            origin: [bounds[0] + 28.0, bounds[1] + 43.0],
            paint: text,
        },
    );
}

fn main() {
    iceblink_examples::run::<Boxes>();
}
