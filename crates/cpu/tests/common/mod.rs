//! Frames built by hand, and the CPU and GPU renderers that draw them.

// Every test binary compiles this module and uses a subset of it.
#![allow(dead_code)]

pub(crate) mod build;
pub(crate) mod content;
pub(crate) mod fault;
pub(crate) mod gpu;
pub(crate) mod targets;

use iceblink::Renderer;
use iceblink::{Frame, FrameOutcome, RenderTarget};
use iceblink_cpu::{CpuRenderer, PixmapPresenter};
use iceblink_testing::golden::RgbaImage;

/// The side of every test output, in pixels.
pub(crate) const SIDE: u32 = 96;

/// Returns a CPU renderer of a `SIDE` square target in kept-target mode.
pub(crate) fn cpu() -> CpuRenderer {
    configured(CpuRenderer::offscreen([SIDE, SIDE]))
}

/// Returns `renderer` configured for a `SIDE` square opaque target at scale 1.
pub(crate) fn configured<R: Renderer>(mut renderer: R) -> R {
    renderer.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    renderer
}

/// Draws `frame` and asserts that it was presented.
pub(crate) fn draw(renderer: &mut dyn Renderer, frame: &Frame) {
    let outcome = renderer.draw(frame);
    assert!(
        matches!(outcome, FrameOutcome::Presented(_)),
        "the frame was not presented: {outcome:?}"
    );
}

/// Returns the image a CPU renderer's pixmap presenter presented last.
pub(crate) fn presented(renderer: &mut CpuRenderer) -> RgbaImage {
    let presenter = renderer
        .presenter()
        .and_then(|presenter| presenter.as_any_mut())
        .and_then(|any| any.downcast_mut::<PixmapPresenter>())
        .expect("the renderer presents into a pixmap presenter");
    let pixmap = presenter.last_presented().expect("a frame was presented");
    RgbaImage::from_pixels(pixmap.width(), pixmap.height(), pixmap.data().to_vec())
}

/// Returns the CPU renderer's drawing of `frame`.
pub(crate) fn on_cpu(frame: &Frame) -> RgbaImage {
    let mut renderer = cpu();
    draw(&mut renderer, frame);
    presented(&mut renderer)
}
