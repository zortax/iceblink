//! The device harness and frame builders the device tests share.

// Every test binary compiles this module and uses a part of it.
#![allow(dead_code)]

pub(crate) mod build;
pub(crate) mod device;
pub(crate) mod pipeline_cache;
pub(crate) mod present;
