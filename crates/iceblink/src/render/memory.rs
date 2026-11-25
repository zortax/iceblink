//! Device memory a renderer holds.

/// Device memory a renderer holds, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryReport {
    /// Memory held from construction on, whatever is drawn.
    pub fixed: u64,
    /// The kept target, pooled group targets and other scratch targets.
    pub targets: u64,
    /// Atlas pages.
    pub atlas: u64,
    /// Resident chunks and per-frame instance buffers.
    pub chunks: u64,
    /// Memory of the path rasteriser.
    pub vector: u64,
}

impl MemoryReport {
    /// Returns the sum of every field.
    pub const fn total(&self) -> u64 {
        self.fixed + self.targets + self.atlas + self.chunks + self.vector
    }

    /// Returns the two reports added field by field.
    #[must_use]
    pub const fn plus(self, other: Self) -> Self {
        Self {
            fixed: self.fixed + other.fixed,
            targets: self.targets + other.targets,
            atlas: self.atlas + other.atlas,
            chunks: self.chunks + other.chunks,
            vector: self.vector + other.vector,
        }
    }
}

/// The pooled group targets a renderer holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TargetPoolReport {
    /// Bytes of pooled targets that exist.
    pub resident: u64,
    /// Bytes of pooled targets lent to the frame being drawn, which cannot be freed.
    ///
    /// Between frames every lease has returned, so it is zero.
    pub lent: u64,
    /// Leases taken since the renderer was built.
    ///
    /// The count never decreases.
    pub leases: u64,
}
