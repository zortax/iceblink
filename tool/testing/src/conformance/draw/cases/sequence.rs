//! Frames that build on the frame before: shifts, partial redraws, resident chunks and opaque
//! covers.

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{Frame, Instance, LoweredChunk, OpaqueCover, PaintRef, Quad, Shift};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, area, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIDE, SIZE, same, skip};
use crate::golden::RgbaImage;

/// Draws white and then seven bars of distinct colours on it, 16 pixels apart and moved up by
/// `lift` pixels.
fn bars(builder: &mut FrameBuilder, paints: &[PaintRef; 8], lift: f32) {
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], paints[0]));
    for (index, paint) in paints[1..].iter().enumerate() {
        let top = 8.0 + index as f32 * 16.0 - lift;
        builder.quad(filled([8.0, top, 112.0, 10.0], *paint));
    }
}

/// Adds white and the colours of the seven bars as paints.
fn bar_paints(builder: &mut FrameBuilder) -> [PaintRef; 8] {
    let colours = [
        [255, 255, 255],
        [200, 30, 30],
        [30, 160, 30],
        [30, 30, 200],
        [200, 160, 0],
        [0, 160, 200],
        [160, 0, 200],
        [80, 80, 80],
    ];
    colours.map(|[r, g, b]| builder.solid(opaque(r, g, b)))
}

/// A frame that shifts the kept target up and draws only the exposed band equals the moved
/// picture drawn whole.
pub fn shifts(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    if !canvas.caps().can_shift {
        skip("shifts", "the renderer cannot shift");
        return;
    }
    let mut first = FrameBuilder::new(SIZE);
    let paints = bar_paints(&mut first);
    bars(&mut first, &paints, 0.0);
    canvas.draw(first.finish());

    let band = area(0, SIDE as i32 - 16, SIDE as i32, 16);
    let mut shifted = FrameBuilder::next(SIZE).damage(band);
    bars(&mut shifted, &paints, 16.0);
    let mut frame = shifted.finish();
    frame.damage.present = DamageSet::full();
    frame.shifts.push(Shift {
        region: Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32)),
        by: [0, -16],
    });
    let moved = canvas.draw(frame);

    let mut whole = FrameBuilder::next(SIZE).damage(DamageSet::full());
    bars(&mut whole, &paints, 16.0);
    let expected = canvas.draw(whole.finish());
    same(
        &expected,
        &moved,
        0,
        "a shifted frame and the same picture drawn whole",
    );
}

/// A frame redraws exactly its draw damage and keeps every other pixel.
pub fn partial_frames(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut first = FrameBuilder::new(SIZE);
    let paints = bar_paints(&mut first);
    bars(&mut first, &paints, 0.0);
    let before = canvas.draw(first.finish());

    let damage = area(20, 30, 50, 40);
    let mut partial = FrameBuilder::next(SIZE).damage(damage);
    bars(&mut partial, &paints, 5.0);
    let drawn = canvas.draw(partial.finish());

    let mut whole = FrameBuilder::next(SIZE).damage(DamageSet::full());
    bars(&mut whole, &paints, 5.0);
    let after = canvas.draw(whole.finish());

    let mut expected = RgbaImage::new(SIDE, SIDE);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let inside = (20..70).contains(&x) && (30..70).contains(&y);
            let source = if inside { &after } else { &before };
            expected.set_pixel(x, y, source.pixel(x, y));
        }
    }
    same(&expected, &drawn, 0, "a partial frame");
}

/// A resident chunk drawn at an offset equals its instances drawn moved from the frame.
pub fn resident_chunks(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    if !canvas.caps().resident_chunks {
        skip("resident_chunks", "the renderer keeps no resident chunks");
        return;
    }
    let offset = [10.5, 20.0];
    let mut builder = FrameBuilder::new(SIZE);
    let paints = bar_paints(&mut builder);
    let quads: Vec<Quad> = (0..4)
        .map(|index| {
            let mut quad = filled(
                [
                    8.0 + index as f32 * 20.0,
                    8.0 + index as f32 * 12.0,
                    30.0,
                    18.0,
                ],
                paints[1 + index],
            );
            quad.radii = [6.0; 8];
            quad
        })
        .collect();
    let chunk = Arc::new(LoweredChunk {
        quads: quads.clone(),
        ..LoweredChunk::default()
    });
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], paints[0]));
    builder.insert(1, chunk.clone());
    builder.draw_resident(1, offset, &chunk);
    let resident = canvas.draw(builder.finish());

    let mut transient = FrameBuilder::next(SIZE).damage(DamageSet::full());
    transient.quad(filled([0.0, 0.0, 128.0, 128.0], paints[0]));
    for quad in &quads {
        transient.quad(quad.moved(offset));
    }
    transient.retire(1);
    let expected = canvas.draw(transient.finish());
    same(
        &expected,
        &resident,
        1,
        "a resident chunk drawn at an offset",
    );
}

/// Returns a first frame of bars on white and a partial frame that lifts them, with an opaque
/// cover over all but the output's edge if `cover` holds.
fn covered(cover: bool) -> (Frame, Frame) {
    let mut first = FrameBuilder::new(SIZE);
    let paints = bar_paints(&mut first);
    bars(&mut first, &paints, 0.0);
    let mut partial = FrameBuilder::next(SIZE).damage(area(16, 16, 64, 64));
    bars(&mut partial, &paints, 3.0);
    let mut partial = partial.finish();
    if cover {
        partial.opaque_covers.push(OpaqueCover {
            rect: Rect::new(Point::new(1, 1), Size::new(126, 126)),
            batch: 0,
            at: 0,
        });
    }
    (first.finish(), partial)
}

/// A frame with an opaque cover draws what the same frame draws without it.
pub fn opaque_covers(make: MakeTarget<'_>) {
    let images = [true, false].map(|cover| {
        let mut canvas = Canvas::open(make)?;
        let (first, partial) = covered(cover);
        canvas.draw(first);
        Some(canvas.draw(partial))
    });
    let [Some(with), Some(without)] = images else {
        return;
    };
    same(&without, &with, 0, "a frame with an opaque cover");
}
