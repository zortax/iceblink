//! The canvas a draw case draws its frames on, and the checks of what it presents.

use std::path::PathBuf;
use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Frame, FrameId, FrameOutcome, PageFormat, PageOp, RenderCapabilities, VectorStatus,
};

use crate::conformance::draw::builder::FrameBuilder;
use crate::conformance::draw::{DrawTarget, MakeTarget};
use crate::golden::{RgbaImage, Tolerance, assert_png, difference};
use crate::validate::{FrameValidator, describe};

/// The side of every case's output, in pixels.
pub(crate) const SIDE: u32 = 128;

/// The size of every case's output.
pub(crate) const SIZE: [u32; 2] = [SIDE, SIDE];

/// A target that draws one case's frames in order.
pub(crate) struct Canvas {
    target: Box<dyn DrawTarget>,
    validator: FrameValidator,
    frames: u64,
}

impl Canvas {
    /// Returns a canvas on a target from `make`, or `None` when the factory has none.
    pub(crate) fn open(make: MakeTarget<'_>) -> Option<Self> {
        let target = make(SIZE)?;
        Some(Self {
            target,
            validator: FrameValidator::new(),
            frames: 0,
        })
    }

    /// Returns the capabilities the renderer reports.
    pub(crate) fn caps(&mut self) -> RenderCapabilities {
        self.target.renderer().capabilities()
    }

    /// Returns the path rasteriser the renderer reports.
    pub(crate) fn vector_status(&mut self) -> VectorStatus {
        self.target.renderer().vector_status()
    }

    /// Draws `frame` and returns the image presented.
    ///
    /// # Panics
    ///
    /// Panics when the frame breaks the frame contract or the renderer does not present it.
    #[track_caller]
    pub(crate) fn draw(&mut self, mut frame: Frame) -> RgbaImage {
        self.frames += 1;
        frame.id = FrameId(self.frames);

        let caps = self.caps();
        self.validator.set_capabilities(Some(caps));
        if let Err(violations) = self.validator.check(&frame) {
            panic!(
                "the case's frame breaks the frame contract:\n{}",
                describe(&violations)
            );
        }

        let outcome = self.target.renderer().draw(&frame);
        assert!(
            matches!(outcome, FrameOutcome::Presented(_)),
            "the renderer did not present the frame: {outcome:?}"
        );
        self.target.presented()
    }
}

/// Reports a case skipped because the renderer lacks what it draws.
pub(crate) fn skip(case: &str, reason: &str) {
    eprintln!("skipped: draw case {case}: {reason}");
}

/// Asserts that `image` matches the golden of `case` under `tolerance`.
#[track_caller]
pub(crate) fn golden(case: &str, image: &RgbaImage, tolerance: Tolerance) {
    let path = [env!("CARGO_MANIFEST_DIR"), "goldens", "draw", case]
        .iter()
        .collect::<PathBuf>()
        .with_extension("png");
    assert_png(&path, image, tolerance);
}

/// Asserts that two images differ by at most `steps` in any channel.
#[track_caller]
pub(crate) fn same(expected: &RgbaImage, actual: &RgbaImage, steps: u8, what: &str) {
    let tolerance = Tolerance {
        max_channel: steps,
        max_pixels: 0,
    };
    let found = difference(expected, actual, tolerance).expect("both images have the case's size");
    assert!(
        found.pixels == 0,
        "{what}: {} pixels differ by more than {steps}, the first at {:?} by {}",
        found.pixels,
        found.first,
        found.max_channel
    );
}

/// Creates page `page` of `format` and `size` in `builder`, and uploads `bytes` over all of it.
pub(crate) fn page(
    builder: &mut FrameBuilder,
    page: u32,
    format: PageFormat,
    size: [u32; 2],
    bytes: Vec<u8>,
) {
    let texel = match format {
        PageFormat::A8 => 1,
        _ => 4,
    };
    builder.page(PageOp::Create {
        page,
        format,
        size,
        levels: 1,
    });
    builder.page(PageOp::Upload {
        page,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(size[0] as i32, size[1] as i32)),
        bytes: Arc::from(bytes),
        offset: 0,
        stride: size[0] * texel,
    });
}
