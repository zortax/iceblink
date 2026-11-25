//! A vector rasteriser for devices without compute shaders.
//!
//! [`CoverageRaster`] implements [`VectorRaster`](crate::vector::VectorRaster) with a vertex and
//! a fragment stage and read-only textures, so it runs on devices that have no compute shaders, no
//! storage buffers or no writable storage textures. Its type documentation lists the limits of
//! what it draws.

mod geometry;
mod instance;
mod pipeline;
mod raster;
mod scratch;

pub use crate::vector_coverage::raster::{CoverageRaster, Rasterised};
