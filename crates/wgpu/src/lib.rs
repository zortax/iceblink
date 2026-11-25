//! The reference GPU renderer: a [`iceblink::Renderer`] that draws a
//! [`iceblink::Frame`] with wgpu, and its presenters.
//!
//! Frames are composed into premultiplied gamma-encoded targets, and no attachment is an `*Srgb`
//! format, so blending happens on the encoded values every target holds. A renderer owns its
//! presenter. In kept-target mode it composes into a target it keeps and copies each acquired
//! image's debt into the image. In buffer-age mode it composes straight into the acquired image.
//!
//! The renderer is built on four public modules:
//!
//! | Module | Contents |
//! | --- | --- |
//! | [`vector`] | The plug-in trait for vector rasterisers that draw path passes into wgpu targets |
//! | [`vector_coverage`] | A vector rasteriser for devices without compute shaders |
//! | [`upload`] | Host-to-device copies through recycled staging buffers, and lookup tables in textures |
//! | [`pipeline_cache`] | Compiled pipelines kept between runs |
//!
//! ```no_run
//! use iceblink::Renderer as _;
//! use iceblink::RenderTarget;
//! use iceblink_wgpu::Builder;
//! use iceblink_wgpu::wgpu::TextureFormat;
//!
//! let mut renderer = Builder::new().offscreen([256, 256], TextureFormat::Bgra8Unorm)?;
//! renderer.configure(RenderTarget { size: [256, 256], scale: 1.0, opaque: true });
//! # Ok::<(), iceblink::GpuUnavailable>(())
//! ```
#![recursion_limit = "256"]

pub mod pipeline_cache;
pub mod upload;
pub mod vector;
pub mod vector_coverage;

mod bind;
mod buffer;
mod effect;
mod filter;
mod frame;
mod gpu;
mod pages;
mod pipeline;
mod present;
mod renderer;
mod shader;
mod target;

pub use wgpu;

pub use crate::effect::check_shader;
pub use crate::gpu::adapter::{gl_enumerations, gl_instances};
#[cfg(target_arch = "wasm32")]
pub use crate::gpu::canvas::Canvas;
pub use crate::gpu::device::{DeviceCapabilities, Gpu};
pub use crate::gpu::formats::{Formats, SrgbTier};
pub use crate::gpu::loader::prefer_hardware_vulkan;
pub use crate::gpu::loss::DeviceLoss;
pub use crate::gpu::pipeline_cache::DIRECTORY_VARIABLE as PIPELINE_CACHE_VARIABLE;
pub use crate::gpu::timing::{
    ENV as GPU_TIMING_VARIABLE, GpuTiming, TimedWork, enable as enable_gpu_timing,
    requested as gpu_timing_requested,
};
pub use crate::present::{
    Acquisition, FRAME_LATENCY, FaultInjector, ImageDebts, Offscreen,
    PRESENTER_VARIABLE as NATIVE_PRESENTER_VARIABLE, PresentPacing, SURFACE_FAULT_VARIABLE,
    Supplied, SurfacePresenter, SwapChain, TIMING_VARIABLE as PRESENT_TIMING_VARIABLE,
    VERIFY_VARIABLE as PRESENT_VERIFY_VARIABLE, WgpuPresenter,
};
pub use crate::renderer::{
    Builder, LatencyMarks, Pixels, PrePresent, PresentGate, ReplacementGpu, SharedGraphics,
    VectorFactory, WgpuRenderer,
};
pub use crate::shader::check_shader_layouts;
pub use crate::target::group_pool::{GroupPool, GroupPrecision};
pub use crate::target::scale::TargetScale;
