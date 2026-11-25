//! The damage of a frame.

use crate::damage::DamageSet;

/// The damage of a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameDamage {
    /// The pixels the renderer redraws, after it applies the shifts.
    pub draw: DamageSet,
    /// The pixels that differ from the previous image presented on the output.
    pub present: DamageSet,
    /// The damage the compositor's own commits derived since the output's previous frame, from
    /// samples of its animations and scroll motion and from nodes that came to rest.
    pub driven: DamageSet,
}
