//! What the renderer plans for each damaged rectangle: where the redraw starts, which batches it
//! leaves out, and what it cannot composite.

mod common;

use std::path::Path;

use common::build::{FrameBuilder, area, filled, group, opaque, rect};
use common::device::{counter, draw, plain_renderer};
use iceblink::geom::{Point, Rect, Size};
use iceblink::{BlendMode, ClipRef, FRAME_NO_CLIP, Line, LineStyle, OpaqueCover, SpaceRef};

// A cover is decided for each damaged rectangle on its own.
#[test]
fn a_rectangle_a_cover_hides_starts_at_the_cover_and_one_it_misses_is_cleared() {
    let mut renderer = plain_renderer();
    // The first frame leaves green everywhere, so a corner that is not cleared stays green.
    let mut first = FrameBuilder::new();
    let green = first.solid(opaque(0, 255, 0));
    first.quad(filled(rect(0.0, 0.0, 128.0, 128.0), green));
    draw(&mut renderer, &first.finish());

    let mut builder = FrameBuilder::new();
    let red = builder.solid(opaque(255, 0, 0));
    let white = builder.solid(opaque(255, 255, 255));
    let blue = builder.solid(opaque(0, 0, 255));
    builder.quad(filled(rect(72.0, 72.0, 48.0, 48.0), red));
    builder.quad(filled(rect(64.0, 64.0, 64.0, 64.0), white));
    builder.quad(filled(rect(80.0, 80.0, 16.0, 16.0), blue));
    let mut damage = area(0, 0, 32, 32);
    damage.absorb_set(&area(80, 80, 32, 32));
    let mut frame = builder.finish();
    frame.damage.draw = damage;
    frame.damage.present = damage;
    frame.opaque_covers.push(OpaqueCover {
        rect: Rect::new(Point::new(64, 64), Size::new(64, 64)),
        batch: 0,
        at: 1,
    });

    let stats = draw(&mut renderer, &frame).stats().expect("presented");

    let pixels = renderer.read_presented().expect("an offscreen image");
    assert_eq!(
        pixels.rgba(16, 16),
        [0, 0, 0, 0],
        "the bare corner is cleared"
    );
    assert_eq!(pixels.rgba(100, 100), [255, 255, 255, 255], "the cover");
    assert_eq!(
        pixels.rgba(88, 88),
        [0, 0, 255, 255],
        "what is drawn over it"
    );
    assert_eq!(
        stats.draw_calls, 4,
        "the corner's clear and batch, the covered rectangle's batch from the cover, and the copy"
    );
}

#[test]
fn a_blend_mode_the_renderer_cannot_composite_is_counted() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(opaque(10, 20, 30));
    let mut multiply = group(rect(16.0, 16.0, 64.0, 64.0), 1.0, &[]);
    multiply.blend = BlendMode::MULTIPLY;
    builder.begin_group(multiply);
    builder.quad(filled(rect(16.0, 16.0, 64.0, 64.0), paint));
    builder.end_group();

    draw(&mut renderer, &builder.finish());

    assert_eq!(counter(&renderer, "blends_unsupported"), 1);

    let mut builder = FrameBuilder::new();
    let paint = builder.solid(opaque(10, 20, 30));
    builder.begin_group(group(rect(16.0, 16.0, 64.0, 64.0), 0.5, &[]));
    builder.quad(filled(rect(16.0, 16.0, 64.0, 64.0), paint));
    builder.end_group();

    draw(&mut renderer, &builder.finish());

    assert_eq!(
        counter(&renderer, "blends_unsupported"),
        1,
        "a source-over group is not counted"
    );
}

/// Returns a background, a quad in the top left, and a stack of four batches in the bottom
/// right that alternate between lines and quads.
fn corner_batches(damage: iceblink::damage::DamageSet) -> iceblink::Frame {
    let mut builder = FrameBuilder::new().damage(damage);
    let paint = builder.solid(opaque(10, 20, 30));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), paint));
    builder.quad(filled(rect(8.0, 8.0, 24.0, 24.0), paint));
    for layer in 0..4 {
        let at = rect(80.0, 80.0 + 2.0 * layer as f32, 32.0, 8.0);
        if layer % 2 == 0 {
            builder.line(Line {
                bounds: at,
                color: opaque(200, 0, 0),
                thickness: 2.0,
                clip: ClipRef(FRAME_NO_CLIP),
                space: SpaceRef(0),
                style: LineStyle::SOLID,
                _pad: [0; 3],
            });
        } else {
            builder.quad(filled(at, paint));
        }
    }
    let frame = builder.finish();
    assert_eq!(frame.batches.len(), 5, "{:?}", frame.batches);
    frame
}

#[test]
fn a_batch_whose_ink_misses_a_damaged_rectangle_is_left_out_of_it() {
    let mut renderer = plain_renderer();
    draw(
        &mut renderer,
        &corner_batches(iceblink::damage::DamageSet::full()),
    );

    let mut two = area(0, 0, 40, 40);
    two.absorb_set(&area(72, 72, 48, 48));

    let stats = draw(&mut renderer, &corner_batches(two))
        .stats()
        .expect("presented");

    assert_eq!(
        stats.batches_culled, 4,
        "the four layers of the stack, once, in the top left"
    );
}

#[test]
fn no_pass_outlives_its_encoder_by_being_told_to_forget_it() {
    let mut found = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );

    assert!(
        found.is_empty(),
        "these discard the encoder's borrow instead of planning round it: {found:?}"
    );
}

/// Appends the Rust sources under `directory` that mention `forget_lifetime` to `found`.
fn walk(directory: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && std::fs::read_to_string(&path).is_ok_and(|text| text.contains("forget_lifetime"))
        {
            found.push(path.display().to_string());
        }
    }
}
