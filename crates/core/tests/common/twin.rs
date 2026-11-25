//! Two compositors given the same commits, one drawn into a kept target and one redrawn whole.

use iceblink::Compositor;
use iceblink::{FrameClock, ScrollGesture, ScrollRouting, Transaction};
use iceblink_testing::golden::{Difference, Tolerance, difference};
use iceblink_testing::gpu::Driver;
use iceblink_wgpu::WgpuRenderer;

use crate::common::gpu::{presented, slice, slice_keeping};
use crate::common::slice::Slice;
use crate::common::story::World;

/// Two slices on a device that commit the same transactions.
///
/// The kept slice builds every frame for its kept target, with shifts, partial damage and kept
/// vector rasters. The whole slice forgets its caches before every frame and keeps no vector
/// raster, so it redraws and presents every pixel and its image is the reference.
pub(crate) struct Twin {
    pub(crate) kept: Slice<WgpuRenderer>,
    pub(crate) whole: Slice<WgpuRenderer>,
}

impl Twin {
    /// Returns a twin past its first frames, or `None` when no device opens.
    pub(crate) fn new() -> Option<Self> {
        let mut twin = Self {
            kept: slice()?,
            whole: slice_keeping(false)?,
        };
        twin.kept.draw();
        twin.whole.draw();
        Some(twin)
    }

    /// Commits what `write` writes into a transaction of each slice, and returns what it returned
    /// for the kept one.
    ///
    /// `write` is called once per slice with its own copy of `state`, so it writes the same
    /// transaction twice when it depends on nothing else. `state` then holds the kept slice's
    /// copy.
    ///
    /// # Panics
    ///
    /// Panics when either commit is rejected.
    pub(crate) fn commit<S: Clone, T>(
        &mut self,
        state: &mut S,
        mut write: impl FnMut(&mut Transaction, &mut World, &mut S) -> T,
    ) -> T {
        let mut copy = state.clone();
        let mut tx = self.whole.tx();
        write(&mut tx, &mut self.whole.world, &mut copy);
        self.whole.compositor.commit(tx).expect("the whole commit");

        let mut tx = self.kept.tx();
        let written = write(&mut tx, &mut self.kept.world, state);
        self.kept.compositor.commit(tx).expect("the kept commit");
        written
    }

    /// Sets the time both slices' next frames are presented at, with frames `interval_ns` apart.
    pub(crate) fn at(&mut self, present_ns: u64, interval_ns: u64) {
        let clock = FrameClock {
            now_ns: present_ns.saturating_sub(interval_ns),
            present_ns,
            interval_ns,
        };
        self.kept.clock = clock;
        self.whole.clock = clock;
    }

    /// Hands one step of a scroll gesture to both compositors, and returns where the kept one
    /// routed it.
    pub(crate) fn input(&mut self, gesture: ScrollGesture) -> ScrollRouting {
        self.whole
            .compositor
            .scroll_input(self.whole.output, gesture);
        self.kept.compositor.scroll_input(self.kept.output, gesture)
    }

    /// Draws the next frame of both slices and returns how the kept image differs from the
    /// whole one.
    ///
    /// Pixels count as different when a channel differs by more than one step, or by more than
    /// two on lavapipe.
    pub(crate) fn draw(&mut self) -> Difference {
        self.kept.draw();
        self.whole.compositor.forget_caches();
        self.whole.draw();
        // Lavapipe rounds blends differently in group targets of another size, and a partial
        // frame draws into smaller ones.
        let lavapipe = Driver::requested() == Driver::Lavapipe;
        let tolerance = Tolerance {
            max_channel: 1 + u8::from(lavapipe),
            max_pixels: 0,
        };
        difference(&presented(&self.whole), &presented(&self.kept), tolerance)
            .expect("the images have one size")
    }
}
