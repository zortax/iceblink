//! The compositor-driven tier: animations the compositor samples, scrolling it drives from
//! gestures, and the reconciliation of both with what clients write.
//!
//! The scene holds the values the compositor shows. The tier keeps what those values are made
//! from: the base values clients wrote, the running animations, and per scroll node the
//! compositor delta and its motion. Before each frame the tier samples them at the frame's
//! present time, and the changed values are written to the scene by a commit of the
//! compositor's own, whose damage and shifts are those of the same client writes.

pub(crate) mod anim;
mod apply;
mod commit;
mod input;
pub(crate) mod scroll;
mod settle;
pub(crate) mod stage;
mod tick;

use crate::scene::record::{EffectRecord, ScrollRecord, TransformRecord};
use crate::scene::slab::Slab;

use self::anim::Animations;
use self::scroll::Scrolls;
use self::scroll::over::Origins;

pub(crate) use self::settle::{settle, settles};
pub(crate) use self::tick::earliest;

/// The compositor-driven state of one client.
#[derive(Debug, Default)]
pub(crate) struct Tier {
    pub(crate) animations: Animations,
    pub(crate) scrolls: Scrolls,
    /// The routing start kept from the last gesture.
    pub(crate) origins: Origins,
    /// The present time of the last frame that sampled the client, which commits stamp their
    /// events with.
    pub(crate) now_ns: u64,
}

/// The parts of a scene the tier reads.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    pub(crate) transforms: &'a Slab<TransformRecord>,
    pub(crate) scrolls: &'a Slab<ScrollRecord>,
    pub(crate) effects: &'a Slab<EffectRecord>,
}
