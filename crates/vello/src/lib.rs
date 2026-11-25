//! A vector rasteriser on the vello compute path renderer.
//!
//! [`VelloRaster`] implements [`iceblink_wgpu::vector::VectorRaster`] with vello. Rasterisers made
//! with [`VelloRaster::sharing`] from one [`SharedRenderer`] share its vello renderer.
//! [`for_device`] returns the rasteriser a device can run: vello where the device has what it
//! needs, and the coverage rasteriser of [`iceblink_wgpu::vector_coverage`] otherwise.

// Proving a wgpu handle `Send` and `Sync` walks a type chain deeper than the default limit.
#![recursion_limit = "256"]

mod device;
mod raster;
mod select;

pub use crate::device::{DIRECTORY_VARIABLE, SharedRenderer};
pub use crate::raster::{Encoded, VelloOptions, VelloRaster};
pub use crate::select::{Choice, chosen, for_device};

/// The vello crate the rasteriser is built on.
pub use vello;
