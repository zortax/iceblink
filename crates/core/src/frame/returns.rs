//! Frame paths that take path masks again once their route history is forgotten.

use iceblink::FrameClock;

use crate::compositor::CompositorImpl;
use crate::content::{ContentCache, PathOwner};
use crate::derive::route::{Routes, damage};

impl CompositorImpl {
    /// Counts a frame of the route histories of paths.
    ///
    /// A frame path whose history is forgotten has its chunk lowered again when next drawn, and
    /// its ink is damaged on the output that draws it.
    pub(super) fn count_route_frame(&mut self) {
        let mut returning = Vec::new();
        self.content.cache.count_frame(&mut returning);
        for owner in returning {
            self.reroute(owner);
        }
    }

    /// Marks the lowering of the chunk of path `owner` stale, and damages the path's ink.
    fn reroute(&mut self, owner: PathOwner) {
        let Some(client) = self.clients.get_mut(&owner.client) else {
            return;
        };
        let scene = &mut client.scene;
        let Some(record) = scene.chunks.get_mut(&owner.chunk) else {
            return;
        };
        if let Some(stamp) = record
            .derived
            .lowered
            .as_mut()
            .and_then(|lowered| lowered.content.as_mut())
        {
            stamp.rerouted = true;
        }

        let routes = Routes::of(scene, &self.outputs);
        let route = routes.of_chunk(scene, owner.chunk);
        let ink = scene.chunks.get(&owner.chunk).and_then(|record| {
            scene
                .device_inks(&record.derived.inks)
                .item(owner.position as usize)
        });
        damage(&mut self.outputs, route, ink);
    }
}

/// Returns when the next frame after one at `clock` is presented, while a frame path of `cache`
/// waits for its route history to be forgotten.
pub(super) fn route_wake(cache: &ContentCache, clock: &FrameClock) -> Option<u64> {
    cache
        .routes_waiting()
        .then(|| clock.present_ns.saturating_add(clock.interval_ns.max(1)))
}
