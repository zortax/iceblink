//! The client suite on the in-process driver, over the reference compositor and over the naive
//! one.

use iceblink::{Compositor, CompositorClient};
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::conformance::client::Rig;

fn stepped(compositor: Box<dyn Compositor>) -> Box<dyn CompositorClient> {
    Box::new(InProcess::new(compositor, DriverOptions::stepped()))
}

mod core {
    use super::*;

    fn compositor() -> Box<dyn Compositor> {
        Box::new(iceblink_core::CompositorImpl::new())
    }

    iceblink_testing::client_suite!(Rig {
        compositor: &compositor,
        client: &stepped,
    });
}

mod naive {
    use super::*;

    fn compositor() -> Box<dyn Compositor> {
        Box::new(iceblink_testing::naive::NaiveCompositor::new())
    }

    iceblink_testing::client_suite!(Rig {
        compositor: &compositor,
        client: &stepped,
    });
}
