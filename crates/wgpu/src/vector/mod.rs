//! The plug-in trait for vector rasterisers that draw path passes into wgpu targets.
//!
//! [`VectorRaster`] is implemented by each rasteriser. [`VectorFrame`] is the work of one round of
//! a frame, and the resource types describe where each pass is drawn. The remaining items are
//! shared by rasterisers: the scratch allocation policy [`Decay`], clip links as outlines, and
//! frame paints as straight colours.

mod decay;
mod frame;
mod keep;
mod raster;
mod resource;

pub mod outline;
pub mod paint;
pub mod space;

pub use self::decay::{Decay, Extent};
pub use self::frame::VectorFrame;
pub use self::keep::Keeping;
pub use self::outline::ClipOutline;
pub use self::raster::VectorRaster;
pub use self::resource::{
    Capacity, Layering, Placement, ResourcedPass, ResourcedPlan, VectorError, VectorTarget,
};
