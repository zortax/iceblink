//! Sampling a client's motion at the present time of a frame.

use iceblink::{ClientId, FrameClock, OutputId};

use crate::compositor::CompositorImpl;
use crate::motion::physics::rest_speed;
use crate::scene::Scene;
use crate::scene::motion::{rest, wake};
use crate::tier::anim::Ends;
use crate::tier::{Tier, View};

impl CompositorImpl {
    /// Samples the animations and scroll motion of the client `output` draws at the frame's
    /// present time, and writes the values that changed to its scene. Transform nodes whose
    /// motion ends at the frame come to rest in the same commit.
    ///
    /// Returns when a later frame next has a sample that changes anything or a transform node
    /// that comes to rest, or `None` when nothing moves.
    pub(crate) fn tick(&mut self, output: OutputId, clock: &FrameClock) -> Option<u64> {
        let (client, _) = self.outputs.get(&output)?.root?;
        let state = self.clients.get_mut(&client)?;
        let rested = rest(&mut state.scene, clock.present_ns);
        let (tier, view) = split(&mut state.scene);
        if !tier.animations.is_active() && tier.scrolls.moving.is_empty() {
            if !rested.is_empty() {
                self.commit_driven(client, Vec::new(), Vec::new(), rested);
            }
            return self.resting(client);
        }

        let now = clock.present_ns.max(tier.now_ns);
        tier.now_ns = now;
        let mut writes = Vec::new();
        let mut changes = Vec::new();
        let mut ends = Ends {
            client,
            ids: &state.ids,
            events: &mut self.events,
            at_ns: now,
        };
        tier.animations
            .sample(&view, now, &mut ends, &mut tier.scrolls, &mut writes);
        let rest_speed = rest_speed(clock.interval_ns);
        tier.scrolls.advance(
            &view,
            client,
            now,
            rest_speed,
            &mut changes,
            &mut self.events,
        );

        let next_frame = now.saturating_add(clock.interval_ns.max(1));
        let moving = tier.scrolls.advancing().then_some(next_frame);
        let wake = earliest(tier.animations.wake(now, clock.interval_ns), moving);

        if !writes.is_empty() || !changes.is_empty() || !rested.is_empty() {
            self.commit_driven(client, writes, changes, rested);
        }
        earliest(wake, self.resting(client))
    }

    /// Returns when the next moving transform node of `client` comes to rest.
    fn resting(&self, client: ClientId) -> Option<u64> {
        wake(&self.clients.get(&client)?.scene)
    }
}

/// Returns whichever of two times comes first, or the one that is set.
pub(crate) fn earliest(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Splits a scene into its tier and the parts the tier reads.
pub(crate) fn split(scene: &mut Scene) -> (&mut Tier, View<'_>) {
    let Scene {
        tier,
        transforms,
        scrolls,
        effects,
        ..
    } = scene;
    (
        tier,
        View {
            transforms,
            scrolls,
            effects,
        },
    )
}
