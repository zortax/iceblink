//! A frame's own damage: what changed, the request's changes, clock-reading content, shifts and
//! the read-extent closure.

use iceblink::damage::DamageSet;
use iceblink::{BufferAge, FrameRequest};

use crate::frame::{closure, sources};
use crate::output::Output;
use crate::scene::Scene;
use crate::shift::{self, Decision};

/// Returns the damage of the next frame of `output` and its shift decisions, and takes the
/// output's pending scrolls.
///
/// `scene` and `root` are the scene and root slot the output draws, if it has a root.
///
/// A changed request or a fresh renderer damages the whole output. Content drawn with a shader
/// that reads the clock is damaged, and the output then wakes for the next frame.
pub(crate) fn own(
    output: &mut Output,
    scene: Option<&Scene>,
    root: Option<u32>,
    request: &FrameRequest<'_>,
) -> (DamageSet, Decision) {
    // A frame built before the last one was reported retires the last one.
    output.flight = None;
    let this_request = (*request.caps, request.subpixel);
    if output.last_request.is_some_and(|last| last != this_request) || output.renderer.fresh {
        output.damage_all();
    }
    output.last_request = Some(this_request);

    let bounds = output.bounds();
    let mut clocked = false;
    if let (Some(scene), Some(root)) = (scene, root) {
        // Content drawn with a shader that reads the clock changes on every frame.
        for ink in sources::timed(scene, root) {
            output.damage(ink);
            clocked = true;
        }
    }
    if clocked {
        let next = request.clock.present_ns + request.clock.interval_ns.max(1);
        output.wake = Some(output.wake.map_or(next, |wake| wake.min(next)));
    }

    let mut own = output.pending;
    let allowed = request.caps.can_shift
        && request.age == BufferAge::Kept
        && output.renderer.kept
        && !output.renderer.fresh;
    let pending = core::mem::take(&mut output.scrolls);
    let decision = shift::decide(scene, root, pending, allowed, bounds, &mut own);

    if let (Some(scene), Some(root)) = (scene, root) {
        let reads = sources::reading(scene, root, bounds);
        closure::expand(&mut own, &reads, bounds);
    }
    (own, decision)
}
