//! Taking scroll gestures for the client an output draws.

use iceblink::{
    AnimationEvent, AnimationTarget, GesturePhase, OutputId, ScrollGesture, ScrollRouting,
};
use smallvec::SmallVec;

use crate::compositor::CompositorImpl;
use crate::tier::anim::Ends;
use crate::tier::scroll::route::candidates;
use crate::tier::scroll::{Input, over};
use crate::tier::tick::split;

impl CompositorImpl {
    /// Routes one step of a scroll gesture on `output` and moves the scroll nodes that take its
    /// delta.
    pub(crate) fn scroll_gesture(
        &mut self,
        output: OutputId,
        gesture: ScrollGesture,
    ) -> ScrollRouting {
        let Some((client, root, interval_ns)) = self.outputs.get(&output).and_then(|output| {
            let (client, root) = output.root?;
            Some((client, root, output.interval_ns))
        }) else {
            return ScrollRouting::Unhandled;
        };
        let velocity = match gesture.phase {
            GesturePhase::End { velocity } => velocity,
            _ => [0.0; 2],
        };

        let finite = gesture
            .position
            .iter()
            .chain(&gesture.delta)
            .chain(&velocity)
            .all(|value| value.is_finite());
        if !finite {
            return ScrollRouting::Unhandled;
        }

        let Some(state) = self.clients.get(&client) else {
            return ScrollRouting::Unhandled;
        };
        let routes = state.scene.tier.scrolls.routes(output, gesture.phase);
        let hit = routes
            .then(|| self.frontmost(output, gesture.position))
            .flatten();

        let Some(state) = self.clients.get_mut(&client) else {
            return ScrollRouting::Unhandled;
        };
        let origin = routes
            .then(|| {
                let mut origins = core::mem::take(&mut state.scene.tier.origins);
                let origin = over::origin(
                    &state.scene,
                    &mut origins,
                    &mut self.work.routing_walks,
                    root.slot(),
                    hit,
                    gesture.position,
                );
                state.scene.tier.origins = origins;
                origin
            })
            .flatten();

        let (tier, view) = split(&mut state.scene);
        let under = origin.map_or_else(SmallVec::new, |space| {
            candidates(&view, space, gesture.position)
        });
        let at_ns = gesture.time_ns.max(tier.now_ns);
        let mut interrupted = SmallVec::<[u32; 2]>::new();
        let node = tier.scrolls.input(
            &view,
            Input {
                output,
                gesture,
                candidates: under,
                at_ns,
                interval_ns,
            },
            &mut interrupted,
        );

        let mut ends = Ends {
            client,
            ids: &state.ids,
            events: &mut self.events,
            at_ns,
        };
        for slot in interrupted {
            tier.animations.end_where(
                |running| {
                    matches!(running.animation.target, AnimationTarget::ScrollOffset(id) if id.slot() == slot)
                },
                AnimationEvent::Replaced,
                &mut ends,
            );
        }

        match node {
            Some(node) => ScrollRouting::Handled { node, client },
            None => ScrollRouting::Unhandled,
        }
    }
}
