//! The graphics device: adapters, device creation, formats, loss, pipeline caching and timing.

pub(crate) mod adapter;
#[cfg(target_arch = "wasm32")]
pub(crate) mod canvas;
pub(crate) mod device;
pub(crate) mod extensions;
pub(crate) mod formats;
pub(crate) mod loader;
pub(crate) mod loss;
pub(crate) mod pipeline_cache;
pub(crate) mod timing;
