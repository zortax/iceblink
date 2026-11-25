//! The present conformance suite on the CPU renderer and its pixmap presenter, in both modes.

mod common;

use iceblink::Renderer;
use iceblink::{PresentMode, SkipReason};
use iceblink_cpu::{CpuRenderer, PixmapPresenter};
use iceblink_testing::conformance::present::PresentRig;
use iceblink_testing::golden::RgbaImage;

use crate::common::fault::Faulty;

/// A CPU renderer whose pixmap presenter can hold presents and time out acquisitions.
struct Rig(CpuRenderer);

impl Rig {
    fn faults(&mut self) -> &mut Faulty {
        self.0
            .presenter()
            .and_then(|presenter| presenter.as_any_mut())
            .and_then(|any| any.downcast_mut::<Faulty>())
            .expect("the renderer presents into a faulty presenter")
    }
}

impl PresentRig for Rig {
    fn renderer(&mut self) -> &mut dyn Renderer {
        &mut self.0
    }

    fn presented(&mut self) -> Option<RgbaImage> {
        let pixmap = self.faults().inner.last_presented()?;
        Some(RgbaImage::from_pixels(
            pixmap.width(),
            pixmap.height(),
            pixmap.data().to_vec(),
        ))
    }

    fn hold_presents(&mut self, held: bool) -> bool {
        self.faults().holding = held;
        true
    }

    fn time_out_acquisitions(&mut self, times: u32) -> bool {
        let faults = self.faults();
        for _ in 0..times {
            faults.acquisitions.push_back(SkipReason::Timeout);
        }
        true
    }

    // A pixmap has no device to lose.
    fn lose_device(&mut self) -> bool {
        false
    }
}

fn rig(mode: PresentMode, size: [u32; 2]) -> Option<Box<dyn PresentRig>> {
    let presenter = match mode {
        PresentMode::KeptTarget => PixmapPresenter::kept(size),
        PresentMode::BufferAge => PixmapPresenter::buffer_age(3, size),
    };
    let renderer = CpuRenderer::new(Box::new(Faulty::new(presenter)));
    Some(Box::new(Rig(renderer)))
}

iceblink_testing::present_suite!(rig);
