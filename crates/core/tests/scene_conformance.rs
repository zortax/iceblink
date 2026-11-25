//! The scene conformance suite on the reference compositor.

use iceblink::Compositor;
use iceblink_core::CompositorImpl;

iceblink_testing::scene_suite!(|| -> Box<dyn Compositor> { Box::new(CompositorImpl::new()) });
