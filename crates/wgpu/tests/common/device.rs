//! Renderers on the test device, one test at a time.

use std::sync::MutexGuard;

use iceblink::Renderer as _;
use iceblink::{Frame, FrameOutcome, RenderTarget};
use iceblink_testing::FrameValidator;
use iceblink_testing::gpu::Driver;
use iceblink_wgpu::{Builder, Pixels, WgpuRenderer, wgpu};

pub(crate) use iceblink_testing::gpu::device_lock;

/// The side of every test output, in pixels.
pub(crate) const SIDE: u32 = 128;

/// A renderer on the test device and the validator its frames pass through.
///
/// The first renderer a test opens holds the device until it drops.
pub(crate) struct TestRenderer {
    pub(crate) renderer: WgpuRenderer,
    pub(crate) validator: FrameValidator,
    _guard: Option<MutexGuard<'static, ()>>,
}

impl std::ops::Deref for TestRenderer {
    type Target = WgpuRenderer;

    fn deref(&self) -> &WgpuRenderer {
        &self.renderer
    }
}

impl std::ops::DerefMut for TestRenderer {
    fn deref_mut(&mut self) -> &mut WgpuRenderer {
        &mut self.renderer
    }
}

/// Returns an unconfigured offscreen renderer of `format` at the side of every test output.
///
/// # Panics
///
/// Panics when no device opens, so a missing device fails the test.
pub(crate) fn offscreen(format: wgpu::TextureFormat) -> WgpuRenderer {
    Builder::new()
        .offscreen([SIDE, SIDE], format)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"))
}

/// Returns a configured offscreen renderer of `format`.
///
/// # Panics
///
/// Panics when no device opens, so a missing device fails the test.
pub(crate) fn renderer(format: wgpu::TextureFormat) -> TestRenderer {
    opened(format, Some(device_lock()))
}

/// Returns a configured offscreen renderer of `Bgra8Unorm`.
pub(crate) fn plain_renderer() -> TestRenderer {
    renderer(wgpu::TextureFormat::Bgra8Unorm)
}

/// Returns two configured offscreen renderers of `Bgra8Unorm` that draw independently.
pub(crate) fn pair() -> (TestRenderer, TestRenderer) {
    let first = plain_renderer();
    let second = opened(wgpu::TextureFormat::Bgra8Unorm, None);
    (first, second)
}

/// Returns a configured offscreen renderer of `format` that holds `guard`.
fn opened(format: wgpu::TextureFormat, guard: Option<MutexGuard<'static, ()>>) -> TestRenderer {
    let mut renderer = offscreen(format);
    eprintln!("adapter: {}", renderer.gpu().describe());
    renderer.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    TestRenderer {
        renderer,
        validator: FrameValidator::new(),
        _guard: guard,
    }
}

/// Returns a bound of `steps` per channel, one step wider on lavapipe.
// lavapipe rounds blends and filtered samples differently.
pub(crate) fn tolerance(steps: u8) -> u8 {
    steps + u8::from(Driver::requested() == Driver::Lavapipe)
}

/// Returns the value of the renderer counter `name`, or zero when the renderer reports none.
pub(crate) fn counter(renderer: &WgpuRenderer, name: &str) -> u64 {
    let mut found = 0;
    renderer.extra_stats(&mut |stat, value| {
        if stat == name {
            found = value;
        }
    });
    found
}

/// Checks `frame` against the frame contract and draws it.
///
/// # Panics
///
/// Panics when the frame breaks the contract.
pub(crate) fn draw(renderer: &mut TestRenderer, frame: &Frame) -> FrameOutcome {
    validated_draw(&mut renderer.renderer, &mut renderer.validator, frame)
}

/// Draws `frame`, asserts it was presented, and reads the presented image back.
pub(crate) fn present(renderer: &mut TestRenderer, frame: &Frame) -> Pixels {
    let outcome = draw(renderer, frame);
    assert!(
        outcome.stats().is_some(),
        "an offscreen frame is always presented, but this one reported {outcome:?}"
    );
    renderer
        .read_presented()
        .expect("an offscreen image can be read back")
}

/// Checks `frame` with `validator` against the frame contract, and draws it with `renderer`.
///
/// A frame that recovers from a device loss resets the validator.
///
/// # Panics
///
/// Panics when the frame breaks the contract.
pub(crate) fn validated_draw(
    renderer: &mut WgpuRenderer,
    validator: &mut FrameValidator,
    frame: &Frame,
) -> FrameOutcome {
    if let Err(violations) = validator.check(frame) {
        panic!(
            "the test frame breaks the frame contract:\n{}",
            iceblink_testing::validate::describe(&violations)
        );
    }
    let outcome = renderer.draw(frame);
    if outcome == FrameOutcome::Recovered {
        validator.device_lost();
    }
    outcome
}
