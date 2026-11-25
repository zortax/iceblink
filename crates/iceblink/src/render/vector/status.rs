//! Which path rasteriser a renderer uses.

/// The kind of path rasteriser a renderer uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorBackend {
    /// A rasteriser in compute shaders on the GPU.
    Compute,
    /// A rasteriser that draws coverage through the GPU's render pipeline.
    Raster,
    /// A rasteriser on the CPU.
    Cpu,
    /// Any other rasteriser.
    Other,
}

/// The path rasteriser a renderer is configured with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VectorStatus {
    /// The kind of the configured rasteriser, if any.
    pub backend: Option<VectorBackend>,
    /// The configured rasteriser's name, empty when none is configured.
    pub name: &'static str,
    /// Whether the rasteriser has been built, which a renderer may defer until the first path.
    pub initialized: bool,
}
