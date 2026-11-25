//! How the compositor scrolls a node in response to input.

use std::sync::Arc;

use crate::bits::bit_set;
use crate::input::ScrollSource;

/// How the compositor scrolls a node in response to input.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollBehaviour {
    /// How motion continues after input.
    pub physics: ScrollPhysics,
    /// The offsets motion comes to rest at.
    pub snap: Arc<[SnapPoint]>,
    /// When motion snaps to a point.
    pub snap_kind: SnapKind,
    /// The sources whose delta past the end of the range displaces the content, which then
    /// springs back. The delta of other sources past the end is dropped.
    pub bounce: Sources,
    /// Whether gestures may scroll each axis, as `[x, y]`.
    pub user_scrollable: [bool; 2],
    /// The sources whose delta this node cannot absorb passes to the next scroll node outwards.
    pub chain: Sources,
}

bit_set! {
    /// A set of the devices scroll gestures come from.
    pub struct Sources(u8) {
        /// A mouse wheel.
        WHEEL = 1 << 0,
        /// A touchpad.
        TOUCHPAD = 1 << 1,
        /// A touch screen.
        TOUCH = 1 << 2,
        /// A keyboard.
        KEYBOARD = 1 << 3,
    }
}

impl Sources {
    /// The empty set.
    pub const NONE: Self = Self(0);
    /// Every source.
    pub const ALL: Self = Self(0b1111);

    /// Returns the set that holds `source` alone.
    pub const fn of(source: ScrollSource) -> Self {
        match source {
            ScrollSource::Wheel => Self::WHEEL,
            ScrollSource::Touchpad => Self::TOUCHPAD,
            ScrollSource::Touch => Self::TOUCH,
            ScrollSource::Keyboard => Self::KEYBOARD,
        }
    }

    /// Reports whether the set holds `source`.
    pub const fn holds(self, source: ScrollSource) -> bool {
        self.contains(Self::of(source))
    }
}

/// How scroll motion continues after input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollPhysics {
    /// Steps animate, and a release flings.
    Kinetic,
    /// Every delta applies immediately, and no motion follows.
    Instant,
}

/// When scroll motion snaps to a snap point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// Always, to the nearest point.
    Mandatory,
    /// Only to a point within a third of the port's extent.
    Proximity,
}

/// An offset scroll motion comes to rest at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapPoint {
    /// The offset, as `[x, y]`. An axis set to NaN does not snap.
    pub offset: [f32; 2],
}
