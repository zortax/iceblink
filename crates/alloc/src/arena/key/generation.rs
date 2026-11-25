//! The occupancy count of a slot.

use core::num::NonZeroU16;

/// The count of a slot's occupants, starting at one.
///
/// Zero is not a generation, so an `Option<Generation>` is two bytes. A slot whose occupant carries
/// [`Generation::LAST`] is retired once the occupant is dropped, and is never reused.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct Generation(NonZeroU16);

impl Generation {
    /// The generation of a slot's first occupant.
    pub const FIRST: Self = match Self::new(1) {
        Some(generation) => generation,
        None => panic!("1 is a valid generation"),
    };

    /// The highest generation.
    pub const LAST: Self = match Self::new(u16::MAX) {
        Some(generation) => generation,
        None => panic!("u16::MAX is a valid generation"),
    };

    /// Wraps a raw generation.
    ///
    /// Returns `None` for zero.
    pub const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the raw generation, which is never zero.
    pub const fn get(self) -> u16 {
        self.0.get()
    }

    /// Returns the generation of the slot's next occupant, or `None` after [`Generation::LAST`].
    ///
    /// ```
    /// use iceblink_alloc::arena::Generation;
    ///
    /// assert_eq!(Generation::FIRST.next(), Generation::new(2));
    /// assert_eq!(Generation::LAST.next(), None);
    /// ```
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}
