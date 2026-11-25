//! Solid colours delivered to a renderer in different ways.

use iceblink::damage::DamageSet;
use iceblink::{Color, FramePaint, PaintKind, PaintRef, Quad};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled};
use crate::conformance::draw::canvas::{Canvas, SIZE, same};
use crate::golden::RgbaImage;

/// The side of one colour's box, in pixels.
const CELL: f32 = 8.0;

/// Returns opaque colours whose red, green and blue lie halfway between two eight-bit steps, and
/// one whose red lies just below the first half.
///
/// Red runs from 0.5 / 255 to 254.5 / 255, so the sweep holds 191.5 / 255.
fn halves() -> Vec<Color> {
    let half = |step: u16| (f32::from(step) + 0.5) / 255.0;
    let mut colours: Vec<Color> = (0..255_u16)
        .map(|step| Color([half(step), half(254 - step), half(step * 7 % 255), 1.0]))
        .collect();
    let below = f32::from_bits(half(0).to_bits() - 1);
    colours.push(Color([below, 0.0, 0.0, 1.0]));
    colours
}

/// Returns the quad of the box of colour `index`, on a grid of 16 boxes a row.
fn cell(index: usize, paint: PaintRef) -> Quad {
    let column = (index % 16) as f32;
    let row = (index / 16) as f32;
    filled([column * CELL, row * CELL, CELL, CELL], paint)
}

/// Draws one box per colour, each painted by its own slot, and returns the image.
fn recorded(canvas: &mut Canvas, colours: &[Color]) -> RgbaImage {
    let mut frame = FrameBuilder::new(SIZE);
    for (index, colour) in colours.iter().enumerate() {
        let paint = frame.solid(*colour);
        frame.quad(cell(index, paint));
    }
    canvas.draw(frame.finish())
}

/// Draws one black box per colour, then rewrites each box's slot to its colour in place, and
/// returns the second image.
fn rewritten(canvas: &mut Canvas, colours: &[Color]) -> RgbaImage {
    let black = vec![Color([0.0, 0.0, 0.0, 1.0]); colours.len()];
    recorded(canvas, &black);

    let mut frame = FrameBuilder::next(SIZE).damage(DamageSet::full());
    for (index, colour) in colours.iter().enumerate() {
        let slot = index as u32;
        let paint = PaintRef {
            kind: PaintKind::SOLID,
            _pad: [0; 3],
            index: slot,
        };
        frame.quad(cell(index, paint));
        frame.set_paint(slot, FramePaint::Solid(*colour));
    }
    canvas.draw(frame.finish())
}

/// A slot rewritten in place to a colour draws what a new slot holding the colour's eight bits
/// draws.
pub fn slot_colours(make: MakeTarget<'_>) {
    let colours = halves();
    // The slot an inline colour packed from each colour lowers to.
    let bytes: Vec<Color> = colours
        .iter()
        .map(|colour| Color::from_rgba8(colour.to_rgba8()))
        .collect();

    // The device lock allows one canvas at a time.
    let Some(live) = Canvas::open(make).map(|mut canvas| rewritten(&mut canvas, &colours)) else {
        return;
    };
    let Some(repaint) = Canvas::open(make).map(|mut canvas| recorded(&mut canvas, &bytes)) else {
        return;
    };
    same(
        &repaint,
        &live,
        0,
        "a slot rewritten in place and a new slot of the colour's eight bits",
    );
}
