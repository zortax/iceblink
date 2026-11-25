//! Canvases lent to groups, backdrops and filter steps, kept between frames for reuse.

use iceblink::geom::{Device, Rect};

use crate::target::{Canvas, Precision};

/// A pool of canvases, and how many leases it has made.
#[derive(Debug, Default)]
pub(crate) struct Pool {
    free: Vec<Canvas>,
    leases: u64,
}

impl Pool {
    /// Returns a transparent canvas over `region`, at half resolution when `half` is set.
    pub(crate) fn acquire(
        &mut self,
        region: Rect<i32, Device>,
        precision: Precision,
        half: bool,
    ) -> Canvas {
        self.leases += 1;
        let mut canvas = self
            .free
            .pop()
            .unwrap_or_else(|| Canvas::new(Rect::ZERO, precision));
        canvas.reset(region, precision, half);
        canvas
    }

    /// Takes `canvas` back for a later lease.
    pub(crate) fn release(&mut self, canvas: Canvas) {
        self.free.push(canvas);
    }

    /// Returns the bytes the pooled canvases take.
    pub(crate) fn bytes(&self) -> u64 {
        self.free.iter().map(Canvas::bytes).sum()
    }

    /// Returns how many canvases the pool has lent since it was made.
    pub(crate) fn leases(&self) -> u64 {
        self.leases
    }

    /// Drops every canvas not lent out, and returns the bytes they held.
    pub(crate) fn release_unused(&mut self) -> u64 {
        let bytes = self.bytes();
        self.free.clear();
        bytes
    }
}
