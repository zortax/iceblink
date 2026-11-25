//! The reference CPU renderer of the shared compositor contract, and its pixmap presenter.
//!
//! [`CpuRenderer`] implements `iceblink::Renderer` on the processor. It draws a `Frame` with the
//! same coverage, distance, gradient, blur and text mathematics as the reference GPU renderer, so
//! the two agree within the conformance tolerance. Vector passes are rasterised with `vello_cpu`.
//!
//! The renderer presents through a [`CpuPresenter`]. [`PixmapPresenter`] presents into pixmaps no
//! window shows, in kept-target mode or in buffer-age mode:
//!
//! ```
//! use iceblink::RenderTarget;
//! use iceblink::Renderer as _;
//! use iceblink_cpu::CpuRenderer;
//!
//! let mut renderer = CpuRenderer::offscreen([640, 480]);
//! renderer.configure(RenderTarget { size: [640, 480], scale: 1.0, opaque: true });
//! assert!(!renderer.capabilities().custom_shaders);
//! ```
//!
//! External textures are bound to the renderer with [`CpuRenderer::bind_external`].

mod draw;
mod external;
mod frame;
mod math;
mod pages;
mod present;
mod renderer;
mod tables;
mod target;
mod vector;

pub use crate::external::ExternalImage;
pub use crate::present::{CpuPresenter, ImageDebts, PixmapPresenter};
pub use crate::renderer::CpuRenderer;
pub use crate::target::Pixmap;
