//! Several outputs on one graphics device.
//!
//! Offscreen renderers stand in for windows. Two renderers from one [`SharedGraphics`] land on one
//! device, draw independently, and move to one replacement device when it is lost.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, device_lock};
use common::present::{Rig, target};
use iceblink::Renderer as _;
use iceblink::{Color, Frame, FrameOutcome};
use iceblink_wgpu::{SharedGraphics, wgpu};

/// Returns two rigs over offscreen renderers from `graphics`, the first holding the device lock.
fn pair(graphics: &SharedGraphics) -> (Rig, Rig) {
    let guard = device_lock();
    let first = graphics
        .renderer_offscreen([SIDE, SIDE], wgpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    let second = graphics
        .renderer_offscreen([SIDE, SIDE], wgpu::TextureFormat::Bgra8Unorm)
        .expect("a device that opened once opens a second renderer");
    (Rig::new(first, guard), Rig::beside(second))
}

/// Returns a frame of `side` filled with `color`.
fn filled_with(color: Color, side: u32) -> Frame {
    let mut builder = FrameBuilder::new();
    builder.frame_mut().size = [side, side];
    let paint = builder.solid(color);
    builder.quad(filled(rect(0.0, 0.0, side as f32, side as f32), paint));
    builder.finish()
}

/// Returns the pixel at the centre of `pixels`.
fn centre(pixels: &iceblink_wgpu::Pixels) -> [u8; 4] {
    pixels.rgba(pixels.size().width / 2, pixels.size().height / 2)
}

#[test]
fn two_outputs_draw_their_own_pixels_on_one_device() {
    let graphics = SharedGraphics::new();
    let (mut first, mut second) = pair(&graphics);

    assert!(Arc::ptr_eq(first.gpu(), second.gpu()));
    assert!(
        graphics
            .gpu()
            .is_some_and(|gpu| Arc::ptr_eq(&gpu, first.gpu())),
        "the shared device is the one the renderers draw on"
    );

    let red = first.present(&filled_with(opaque(255, 0, 0), SIDE));
    let blue = second.present(&filled_with(opaque(0, 0, 255), SIDE));

    assert_eq!(centre(&red), [255, 0, 0, 255]);
    assert_eq!(centre(&blue), [0, 0, 255, 255]);
}

#[test]
fn outputs_of_different_formats_share_the_device() {
    let guard = device_lock();
    let graphics = SharedGraphics::new();
    let first = graphics
        .renderer_offscreen([SIDE, SIDE], wgpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    let second = graphics
        .renderer_offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm)
        .expect("a second format opens no second device");

    assert!(Arc::ptr_eq(first.gpu(), second.gpu()));

    let _first = Rig::new(first, guard);
    let mut second = Rig::beside(second);
    assert_eq!(
        centre(&second.present(&filled_with(opaque(0, 255, 0), SIDE))),
        [0, 255, 0, 255]
    );
}

#[test]
fn resizing_one_output_leaves_the_other_at_its_own_size() {
    let graphics = SharedGraphics::new();
    let (mut first, mut second) = pair(&graphics);
    first.configure(target(SIDE / 2));
    let small = first.present(&filled_with(opaque(255, 0, 0), SIDE / 2));
    let large = second.present(&filled_with(opaque(0, 0, 255), SIDE));

    assert_eq!(small.size().width, SIDE as i32 / 2);
    assert_eq!(
        large.size().width,
        SIDE as i32,
        "one output's resize resized another"
    );
    assert_eq!(centre(&large), [0, 0, 255, 255]);
}

#[test]
fn a_lost_device_is_replaced_once_for_every_output_on_it() {
    let graphics = SharedGraphics::new();
    let (mut first, mut second) = pair(&graphics);
    let dead = Arc::as_ptr(first.gpu());
    first
        .gpu()
        .loss()
        .report(wgpu::DeviceLostReason::Unknown, "injected by a test");

    assert!(
        second.gpu().loss().is_lost(),
        "one loss is seen by every output"
    );

    let white = || filled_with(opaque(255, 255, 255), SIDE);

    assert_eq!(first.draw(&white()), FrameOutcome::Recovered);
    assert_eq!(second.draw(&white()), FrameOutcome::Recovered);
    assert!(
        Arc::ptr_eq(first.gpu(), second.gpu()),
        "the outputs came out of the loss on different devices"
    );
    assert_ne!(Arc::as_ptr(first.gpu()), dead);
    assert_eq!(centre(&first.present(&white())), [255, 255, 255, 255]);
}

#[test]
fn with_every_backend_masked_out_no_output_gets_a_device() {
    let _guard = device_lock();
    let graphics = SharedGraphics::with_backends(wgpu::Backends::empty());
    let failure = graphics
        .renderer_offscreen([SIDE, SIDE], wgpu::TextureFormat::Bgra8Unorm)
        .expect_err("no backend can produce a device");

    assert!(
        failure.candidates.is_empty(),
        "nothing was enumerated, so nothing was rejected: {:?}",
        failure.candidates
    );
    assert!(
        graphics.gpu().is_none(),
        "a failed open left a device behind"
    );
}
