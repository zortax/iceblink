//! The tier conformance suite on the naive compositor, which reports no tier feature.

use iceblink::Compositor;
use iceblink_testing::naive::NaiveCompositor;

iceblink_testing::tier_suite!(|| -> Box<dyn Compositor> { Box::new(NaiveCompositor::new()) });
