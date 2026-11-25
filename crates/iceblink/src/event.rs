//! Events the compositor queues for its clients.

use crate::chunk::ChunkKey;
use crate::damage::DamageSet;
use crate::id::{AnimationId, ClientId, OutputId, ScrollId};

/// Something that happened in the compositor, queued in the order it happened.
#[derive(Clone, Debug, PartialEq)]
pub enum CompositorEvent {
    /// An animation reached a point of its life.
    Animation {
        /// The client that started the animation.
        client: ClientId,
        /// The animation that reached the point.
        id: AnimationId,
        /// What happened.
        event: AnimationEvent,
        /// When it happened, on the frame clock in nanoseconds.
        at_ns: u64,
    },
    /// The compositor moved a scroll node.
    Scroll {
        /// The client that owns the node.
        client: ClientId,
        /// The node that moved.
        node: ScrollId,
        /// The offset the node shows.
        offset: [f32; 2],
        /// The overscroll displacement.
        overscroll: [f32; 2],
        /// A number that increases with every scroll event of the node.
        serial: u64,
        /// The offset that input and physics have added to the client's base offset, at this
        /// serial.
        delta: [f32; 2],
        /// What moved the node.
        phase: ScrollPhase,
    },
    /// A recording changed items outside the damage hint it declared.
    HintViolated {
        /// The client that recorded the chunk.
        client: ClientId,
        /// The chunk whose recording broke its hint.
        chunk: ChunkKey,
    },
    /// A commit's declared damage missed pixels its changes damage.
    DamageUndeclared {
        /// The client that committed.
        client: ClientId,
        /// The output the declaration was for.
        output: OutputId,
        /// The damage the compositor derived for the commit.
        derived: DamageSet,
    },
}

/// A point in the life of an animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationEvent {
    /// The first sample in the active phase.
    Started,
    /// An iteration boundary.
    Iteration,
    /// The end of the active phase.
    Finished,
    /// An explicit write or a new animation of the target ended the animation.
    Replaced,
    /// The animation was cancelled, or its target was removed.
    Cancelled,
}

/// What moved a scroll node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollPhase {
    /// Gesture input moved the node.
    Gesture,
    /// A discrete step, such as a wheel notch, or the curve of a kinetic step moved the node.
    Step,
    /// A fling or the platform's momentum moved the node.
    Momentum,
    /// Motion towards a snap point moved the node.
    Snap,
    /// The node's motion came to rest.
    Settled,
    /// An animation of the node's offset moved it.
    Programmatic,
}
