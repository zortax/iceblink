//! The draw conformance suite on the CPU renderer.

mod common;

use crate::common::targets::on_cpu;

iceblink_testing::draw_suite!(on_cpu);
