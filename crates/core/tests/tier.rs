//! The tier conformance suite on the reference compositor.

mod common;

use iceblink::Compositor;

use crate::common::checked;

iceblink_testing::tier_suite!(|| -> Box<dyn Compositor> { Box::new(checked()) });
