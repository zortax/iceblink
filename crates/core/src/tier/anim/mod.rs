//! The animations of one client, and the base values of the nodes they animate.

pub(crate) mod effect;
mod sample;

use iceblink::{
    Animation, AnimationEvent, AnimationId, AnimationStart, AnimationTarget, AnyId, ClientId,
    CompositorEvent, EffectId, EffectNode, IdSpace, Matrix4, TransformId,
};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::motion::value::Value;

pub(crate) use self::sample::Write;

/// A node an animation drives a property of, by slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Node {
    Transform(u32),
    Effect(u32),
    Scroll(u32),
}

impl Node {
    /// Returns the node `target` drives a property of.
    pub(crate) fn of(target: &AnimationTarget) -> Self {
        match *target {
            AnimationTarget::Transform(id) => Self::Transform(id.slot()),
            AnimationTarget::Opacity(id) | AnimationTarget::Filter { effect: id, .. } => {
                Self::Effect(id.slot())
            }
            AnimationTarget::ScrollOffset(id) => Self::Scroll(id.slot()),
        }
    }
}

/// The value a client last wrote to an animated node, which returns once no animation drives
/// it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Base {
    Transform(TransformId, Matrix4),
    Effect(EffectId, EffectNode),
    /// A scroll node, whose base offset its scroll state keeps.
    Scroll,
}

/// An animation the compositor samples.
#[derive(Debug)]
pub(crate) struct Running {
    pub(crate) id: AnimationId,
    pub(crate) animation: Animation,
    /// When the animation's time starts, once the first frame that samples it has resolved it.
    pub(crate) start_ns: Option<u64>,
    /// Whether `Started` was emitted.
    pub(crate) started: bool,
    /// Whether `Finished` was emitted.
    pub(crate) finished: bool,
    /// The iteration of the last sample.
    pub(crate) iteration: u64,
    /// The value of the last sample, or `None` when it applied none.
    pub(crate) value: Option<Value>,
}

/// The animations of one client.
#[derive(Debug, Default)]
pub(crate) struct Animations {
    pub(crate) running: Vec<Running>,
    pub(crate) bases: FxHashMap<Node, Base>,
    /// The nodes whose value the next frame writes again, since an animation of them ended.
    pub(crate) dirty: SmallVec<[Node; 4]>,
}

/// Where the events and the freed ids of ended animations go.
pub(crate) struct Ends<'a> {
    pub(crate) client: ClientId,
    pub(crate) ids: &'a IdSpace,
    pub(crate) events: &'a mut Vec<CompositorEvent>,
    /// The time the events happen at.
    pub(crate) at_ns: u64,
}

impl Ends<'_> {
    /// Emits `event` for animation `id` and frees its id.
    pub(crate) fn end(&mut self, id: AnimationId, event: AnimationEvent) {
        self.events.push(CompositorEvent::Animation {
            client: self.client,
            id,
            event,
            at_ns: self.at_ns,
        });
        self.ids.free(&[AnyId::Animation(id)]);
    }
}

impl Animations {
    /// Starts an animation, replacing the running animation of its target.
    ///
    /// `base` is the target node's value as the client last wrote it, kept unless an earlier
    /// animation of the node already keeps one.
    pub(crate) fn start(
        &mut self,
        id: AnimationId,
        animation: Animation,
        base: Base,
        ends: &mut Ends<'_>,
    ) {
        let target = animation.target;
        self.end_where(
            |running| running.animation.target == target,
            AnimationEvent::Replaced,
            ends,
        );

        let node = Node::of(&target);
        self.bases.entry(node).or_insert(base);
        self.mark(node);

        let start_ns = match animation.start {
            AnimationStart::At(at) => Some(at),
            AnimationStart::Next => None,
        };
        self.running.push(Running {
            id,
            animation,
            start_ns,
            started: false,
            finished: false,
            iteration: 0,
            value: None,
        });
    }

    /// Cancels the running animation `id`, if it runs.
    pub(crate) fn cancel(&mut self, id: AnimationId, ends: &mut Ends<'_>) {
        self.end_where(|running| running.id == id, AnimationEvent::Cancelled, ends);
    }

    /// Ends every animation of `node` with `event`, and forgets the node's base, for a node the
    /// client wrote or removed.
    pub(crate) fn release(&mut self, node: Node, event: AnimationEvent, ends: &mut Ends<'_>) {
        self.end_where(
            |running| Node::of(&running.animation.target) == node,
            event,
            ends,
        );
        self.bases.remove(&node);
        self.dirty.retain(|dirty| *dirty != node);
    }

    /// Ends the running animations `ended` selects with `event`, and marks their nodes for the
    /// next frame to write.
    pub(crate) fn end_where(
        &mut self,
        mut ended: impl FnMut(&Running) -> bool,
        event: AnimationEvent,
        ends: &mut Ends<'_>,
    ) {
        let mut index = 0;
        while index < self.running.len() {
            if ended(&self.running[index]) {
                let running = self.running.swap_remove(index);
                ends.end(running.id, event);
                self.mark(Node::of(&running.animation.target));
            } else {
                index += 1;
            }
        }
    }

    /// Reports whether a frame has an unfinished animation to sample or a node to write again.
    pub(crate) fn is_active(&self) -> bool {
        !self.dirty.is_empty() || self.running.iter().any(|running| !running.finished)
    }

    /// Marks a node for the next frame to write.
    fn mark(&mut self, node: Node) {
        if !self.dirty.contains(&node) {
            self.dirty.push(node);
        }
    }
}
