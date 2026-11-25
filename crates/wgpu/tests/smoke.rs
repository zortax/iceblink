//! A known pattern composed, copied and read back on the device.

mod common;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, draw, plain_renderer, present};
use iceblink::Renderer as _;
use iceblink::{BorderStyle, Frame};

/// Adds a red field, a green square over its middle, and a blue border round the square.
fn pattern(builder: &mut FrameBuilder) {
    let field = builder.solid(opaque(255, 0, 0));
    let square = builder.solid(opaque(0, 255, 0));
    let border = builder.solid(opaque(0, 0, 255));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), field));
    let mut bordered = filled(rect(32.0, 32.0, 64.0, 64.0), square);
    bordered.border = [8.0; 4];
    bordered.stroke = border;
    bordered.style = BorderStyle::SOLID;
    builder.quad(bordered);
}

fn first_frame() -> Frame {
    let mut builder = FrameBuilder::new();
    pattern(&mut builder);
    builder.finish()
}

// A later frame writes no table, so its paints refer to the slots the first frame wrote.
fn later_frame() -> Frame {
    let mut builder = FrameBuilder::next();
    pattern(&mut builder);
    builder.finish()
}

#[test]
fn a_known_pattern_survives_composition_and_the_copy() {
    let mut renderer = plain_renderer();
    let pixels = present(&mut renderer, &first_frame());

    assert_eq!(pixels.rgba(4, 4), [255, 0, 0, 255], "the field");
    assert_eq!(pixels.rgba(64, 64), [0, 255, 0, 255], "the square");
    assert_eq!(pixels.rgba(64, 36), [0, 0, 255, 255], "the border");
    assert_eq!(
        pixels.rgba(64, 28),
        [255, 0, 0, 255],
        "just outside the square"
    );

    let size = pixels.size();
    let bytes = pixels.bytes();
    assert_eq!(bytes.len(), (size.width * size.height * 4) as usize);
    for y in 0..size.height {
        for x in 0..size.width {
            let offset = ((y * size.width + x) * 4) as usize;
            let raw: [u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
            let expected = if pixels.is_bgra() {
                [raw[2], raw[1], raw[0], raw[3]]
            } else {
                raw
            };
            assert_eq!(pixels.rgba(x, y), expected, "the bytes at ({x}, {y})");
        }
    }
}

#[test]
fn what_was_composed_is_what_was_presented() {
    let mut renderer = plain_renderer();
    let presented = present(&mut renderer, &first_frame());

    assert_eq!(renderer.read_composed().max_difference(&presented), 0);
}

#[test]
fn a_frame_reports_what_it_drew() {
    let mut renderer = plain_renderer();
    let stats = draw(&mut renderer, &first_frame())
        .stats()
        .expect("the frame was presented");

    assert_eq!(
        stats.draw_calls, 3,
        "the clear, one batch of two quads, and the copy"
    );
    assert_eq!(stats.damage_px, u64::from(SIDE * SIDE));
    assert_eq!(stats.present_copy_px, u64::from(SIDE * SIDE));
    assert!(stats.bytes_uploaded > 0);
    assert!(
        renderer.memory().targets > 0,
        "the kept target is held between frames"
    );
}

#[test]
fn a_second_frame_of_the_same_content_is_identical_and_uploads_less() {
    let mut renderer = plain_renderer();
    let first = draw(&mut renderer, &first_frame()).stats().unwrap();
    let pixels = renderer.read_presented().unwrap();

    let second = draw(&mut renderer, &later_frame()).stats().unwrap();

    assert_eq!(
        renderer.read_presented().unwrap().max_difference(&pixels),
        0
    );
    assert!(
        second.bytes_uploaded < first.bytes_uploaded,
        "the unchanged frame uploads instances, not tables ({} against {})",
        second.bytes_uploaded,
        first.bytes_uploaded
    );
}
