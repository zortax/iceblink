//! Applying a successful client commit to the tier.

use iceblink::{
    AnimationEvent, AnimationId, AnimationTarget, AnyId, ClientId, Command, CompositorEvent,
    IdSpace,
};
use smallvec::SmallVec;

use crate::scene::Scene;
use crate::tier::anim::{Base, Ends, Node};
use crate::tier::scroll::state::Axis;
use crate::tier::stage::TierStage;

/// Applies the tier changes of a successful commit of `commands`, and returns the animations it
/// started.
///
/// The commands `stage` lists take effect in order:
///
/// - A write of a transform or an effect node ends its animations with `Replaced`.
/// - A scroll offset write ends the node's offset animation with `Replaced` and its motion.
/// - `animate` starts an animation, and `cancel_animation` or removing the animation's id
///   cancels it.
/// - Removing a node cancels its animations and forgets its scroll state.
///
/// The staged base offsets and compositor deltas then replace those of their nodes.
pub(crate) fn settle(
    scene: &mut Scene,
    ids: &IdSpace,
    commands: &[Command],
    stage: TierStage,
    events: &mut Vec<CompositorEvent>,
) -> SmallVec<[AnimationId; 2]> {
    let idle = scene.tier.animations.running.is_empty()
        && scene.tier.animations.bases.is_empty()
        && scene.tier.scrolls.nodes.is_empty();
    if idle && !stage.animates && stage.scrolls.is_empty() {
        return SmallVec::new();
    }
    let client: ClientId = scene.client;
    let Scene {
        tier,
        transforms,
        effects,
        scrolls,
        ..
    } = scene;
    let mut ends = Ends {
        client,
        ids,
        events,
        at_ns: tier.now_ns,
    };
    let animations = &mut tier.animations;
    let mut started = SmallVec::new();

    for command in stage.commands.iter().map(|&at| &commands[at as usize]) {
        match command {
            Command::SetTransform(id, _) | Command::SetTransformNode(id, _) => {
                animations.release(
                    Node::Transform(id.slot()),
                    AnimationEvent::Replaced,
                    &mut ends,
                );
            }
            Command::SetEffect(id, _) => {
                animations.release(Node::Effect(id.slot()), AnimationEvent::Replaced, &mut ends);
            }
            Command::ScrollTo(id, _) => {
                animations.release(Node::Scroll(id.slot()), AnimationEvent::Replaced, &mut ends);
            }
            Command::SetScrollNode(id, _)
                if stage.scroll(*id).is_some_and(|staged| staged.reset) =>
            {
                animations.release(Node::Scroll(id.slot()), AnimationEvent::Replaced, &mut ends);
            }
            Command::CreateScroll(id, _) => tier.scrolls.forget(id.slot()),
            Command::Animate(id, animation) => {
                let base = match animation.target {
                    AnimationTarget::Transform(target) => transforms
                        .get(target.slot(), target.generation())
                        .map(|record| Base::Transform(target, record.node.local)),
                    AnimationTarget::Opacity(target)
                    | AnimationTarget::Filter { effect: target, .. } => effects
                        .get(target.slot(), target.generation())
                        .map(|record| Base::Effect(target, record.node.clone())),
                    AnimationTarget::ScrollOffset(_) => Some(Base::Scroll),
                };
                match base {
                    Some(base) => animations.start(*id, (**animation).clone(), base, &mut ends),
                    // The commit removed the target after it started the animation.
                    None => ends.end(*id, AnimationEvent::Cancelled),
                }
                started.push(*id);
            }
            Command::CancelAnimation(id) | Command::Remove(AnyId::Animation(id)) => {
                animations.cancel(*id, &mut ends);
            }
            Command::Remove(AnyId::Transform(id)) => {
                animations.release(
                    Node::Transform(id.slot()),
                    AnimationEvent::Cancelled,
                    &mut ends,
                );
            }
            Command::Remove(AnyId::Effect(id)) => {
                animations.release(
                    Node::Effect(id.slot()),
                    AnimationEvent::Cancelled,
                    &mut ends,
                );
            }
            Command::Remove(AnyId::Scroll(id)) => {
                animations.release(
                    Node::Scroll(id.slot()),
                    AnimationEvent::Cancelled,
                    &mut ends,
                );
                tier.scrolls.forget(id.slot());
            }
            _ => {}
        }
    }

    for staged in stage.scrolls {
        let Some(record) = scrolls.get(staged.id.slot(), staged.id.generation()) else {
            continue;
        };
        let shown = (record.node.offset, record.overscroll);
        let state = tier.scrolls.state(staged.id, staged.base);
        state.base = staged.base;
        for (axis, held) in state.axes.iter_mut().enumerate() {
            held.motion.shift(staged.delta[axis] - state.delta[axis]);
        }
        state.delta = staged.delta;
        if staged.reset {
            state.programmatic = None;
            state.axes = [Axis::default(); 2];
            state.serials.reset();
            // The client's write ends the motion, so no event follows it.
            state.unsettled = false;
            state.shown = (shown.0, [0.0; 2]);
        } else {
            if let Some(serial) = staged.acknowledged {
                state.serials.acknowledge(serial);
            }
            state.shown.0 = shown.0;
        }
        if staged.reset && shown.1 != [0.0; 2] {
            // The next frame takes the displacement away.
            tier.scrolls.wake(staged.id.slot());
        }
    }
    started
}

/// Reports whether `command` changes the tier once its commit succeeds.
pub(crate) fn settles(command: &Command) -> bool {
    matches!(
        command,
        Command::SetTransform(..)
            | Command::SetTransformNode(..)
            | Command::SetEffect(..)
            | Command::ScrollTo(..)
            | Command::SetScrollNode(..)
            | Command::CreateScroll(..)
            | Command::Animate(..)
            | Command::CancelAnimation(_)
            | Command::Remove(
                AnyId::Animation(_) | AnyId::Transform(_) | AnyId::Effect(_) | AnyId::Scroll(_)
            )
    )
}
