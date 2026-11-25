//! The scene conformance suite on the naive compositor.

use iceblink::Compositor;
use iceblink_testing::naive::NaiveCompositor;

iceblink_testing::scene_suite!(|| -> Box<dyn Compositor> { Box::new(NaiveCompositor::new()) });
