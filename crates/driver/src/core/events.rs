//! The routing of the compositor's events to their clients, with the watches the events fire.

use std::collections::HashMap;

use iceblink::smallvec::SmallVec;
use iceblink::{ClientEvent, ClientId, CompositorEvent, ScrollId, ScrollPhase, WatchCause};

use crate::core::Driver;
use crate::core::motion::Motion;
use crate::core::watch::Watched;
use crate::mailbox::Letter;

/// What wakes one client, and the motion of its scroll nodes.
#[derive(Default)]
pub(super) struct Watches {
    pub(super) animations: bool,
    pub(super) scroll: HashMap<ScrollId, Watched>,
    /// The motion of each scroll node that sent a scroll event, by the node's slot.
    motions: HashMap<u32, (ScrollId, Motion)>,
}

impl Watches {
    /// Takes a scroll event of `node` at `offset`, which settles the node when `settled` holds,
    /// and returns what the node's watch fired and the node's motion step.
    fn scrolled(
        &mut self,
        node: ScrollId,
        offset: [f32; 2],
        settled: bool,
    ) -> (SmallVec<[WatchCause; 2]>, [f32; 2]) {
        let (id, motion) = self
            .motions
            .entry(node.slot())
            .or_insert((node, Motion::default()));
        // A node made in a freed slot starts without motion.
        if *id != node {
            *id = node;
            *motion = Motion::default();
        }

        let last = motion.observe(offset, settled);
        let step = motion.step();
        let fired = self
            .scroll
            .get_mut(&node)
            .map(|watched| watched.check(offset, last, step, settled))
            .unwrap_or_default();
        (fired, step)
    }
}

impl Driver {
    /// Addresses the compositor's queued events to their clients, with the watches they fire.
    pub(super) fn route_events(&mut self) {
        self.compositor.drain_events(&mut self.events);
        for event in self.events.drain(..) {
            let client = client_of(&event);
            let watches = self.clients.get_mut(&client);
            let wakes = match &event {
                CompositorEvent::Scroll { .. } => false,
                CompositorEvent::Animation { .. } => watches.as_ref().is_some_and(|w| w.animations),
                CompositorEvent::HintViolated { .. } | CompositorEvent::DamageUndeclared { .. } => {
                    true
                }
            };

            let mut fired = SmallVec::<[_; 2]>::new();
            if let CompositorEvent::Scroll {
                node,
                offset,
                serial,
                phase,
                ..
            } = &event
                && let Some(watches) = watches
            {
                let (causes, step) =
                    watches.scrolled(*node, *offset, *phase == ScrollPhase::Settled);
                for cause in causes {
                    fired.push(ClientEvent::Watched {
                        node: *node,
                        cause,
                        offset: *offset,
                        serial: *serial,
                        step,
                    });
                }
            }

            self.letters.push(Letter {
                client,
                event: ClientEvent::Compositor(event),
                wakes,
            });
            for event in fired {
                self.letters.push(Letter {
                    client,
                    event,
                    wakes: true,
                });
            }
        }
    }

    pub(super) fn send(&mut self, client: ClientId, event: ClientEvent, wakes: bool) {
        self.letters.push(Letter {
            client,
            event,
            wakes,
        });
    }
}

fn client_of(event: &CompositorEvent) -> ClientId {
    match event {
        CompositorEvent::Animation { client, .. }
        | CompositorEvent::Scroll { client, .. }
        | CompositorEvent::HintViolated { client, .. }
        | CompositorEvent::DamageUndeclared { client, .. } => *client,
    }
}
