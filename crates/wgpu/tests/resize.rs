//! Resizes and scale changes: what the kept target keeps across a configure.

mod common;

use common::build::{FrameBuilder, area, filled, opaque, rect};
use common::device::{SIDE, TestRenderer, draw, plain_renderer, present};
use iceblink::Renderer as _;
use iceblink::damage::DamageSet;
use iceblink::{BufferAge, Frame, RenderTarget};

/// Returns an opaque square target of `side` pixels at `scale`.
fn target(side: u32, scale: f32) -> RenderTarget {
    RenderTarget {
        size: [side, side],
        scale,
        opaque: true,
    }
}

/// Returns a white output of `side` with a red square in its top left.
fn resized(side: u32, damage: DamageSet) -> Frame {
    let mut builder = FrameBuilder::sized(side, side).damage(damage);
    let white = builder.solid(opaque(255, 255, 255));
    let red = builder.solid(opaque(200, 40, 40));
    builder.quad(filled(rect(0.0, 0.0, side as f32, side as f32), white));
    builder.quad(filled(rect(8.0, 8.0, 40.0, 40.0), red));
    builder.finish()
}

/// Returns the bands a grown output exposes past the old side.
fn exposed(from: i32, to: i32) -> DamageSet {
    let mut bands = area(from, 0, to - from, to);
    bands.absorb_set(&area(0, from, from, to - from));
    bands
}

/// Draws `side` whole and asserts the kept target already held that picture.
fn assert_kept(renderer: &mut TestRenderer, side: u32) {
    let kept = renderer.read_composed();
    present(renderer, &resized(side, DamageSet::full()));
    assert_eq!(
        kept.max_difference(&renderer.read_composed()),
        0,
        "the kept target holds the frame drawn whole"
    );
}

// A target that grows inside its size class or is reallocated keeps its picture, so the next frame
// redraws only what the new size exposes.
#[test]
fn a_resized_target_keeps_its_picture() {
    let mut renderer = plain_renderer();
    present(&mut renderer, &resized(SIDE, DamageSet::full()));

    renderer.configure(target(300, 1.0));

    assert_eq!(renderer.target_age(), BufferAge::Kept);
    let bands = exposed(SIDE as i32, 300);

    let stats = draw(&mut renderer, &resized(300, bands))
        .stats()
        .expect("presented");

    assert_eq!(stats.damage_px, 300 * 300 - u64::from(SIDE * SIDE));
    assert_kept(&mut renderer, 300);

    renderer.configure(target(500, 1.0));
    renderer.configure(target(600, 1.0));
    draw(&mut renderer, &resized(600, exposed(300, 600)));
    assert_kept(&mut renderer, 600);

    renderer.configure(target(100, 1.0));
    draw(&mut renderer, &resized(100, DamageSet::new()));
    assert_kept(&mut renderer, 100);
}

/// Returns the width of the image the offscreen presenter shows frames in.
fn image_width(renderer: &mut TestRenderer) -> u32 {
    renderer
        .renderer
        .wgpu_presenter()
        .as_any_mut()
        .and_then(|any| any.downcast_mut::<iceblink_wgpu::Offscreen>())
        .and_then(|offscreen| offscreen.texture(0).map(wgpu::Texture::width))
        .expect("the renderer presents into an offscreen image")
}

// The presenter's image is kept while the output stays inside its size class, and a frame is read
// back at the output's size.
#[test]
fn a_resize_inside_the_size_class_keeps_the_presented_image() {
    let mut renderer = plain_renderer();
    present(&mut renderer, &resized(SIDE, DamageSet::full()));
    renderer.configure(target(SIDE + 8, 1.0));
    let width = image_width(&mut renderer);
    renderer.configure(target(SIDE + 16, 1.0));

    assert_eq!(image_width(&mut renderer), width, "the image is kept");

    let side = SIDE + 16;
    let pixels = present(
        &mut renderer,
        &resized(side, exposed(SIDE as i32, side as i32)),
    );

    assert_eq!(pixels.size().width, side as i32);
    assert_eq!(pixels.rgba(10, 10), [200, 40, 40, 255], "the kept square");
    assert_eq!(
        pixels.rgba(side as i32 - 1, side as i32 - 1),
        [255, 255, 255, 255],
        "the exposed corner"
    );
}

#[test]
fn a_scale_change_redraws_the_whole_output_whatever_the_frame_s_damage() {
    let mut renderer = plain_renderer();
    present(&mut renderer, &resized(SIDE, DamageSet::full()));
    renderer.configure(target(SIDE, 2.0));
    assert_eq!(renderer.target_age(), BufferAge::Unknown);
    let mut frame = resized(SIDE, area(0, 0, 16, 16));
    frame.scale = 2.0;

    let stats = draw(&mut renderer, &frame).stats().expect("presented");

    assert_eq!(stats.damage_px, u64::from(SIDE * SIDE));
}
