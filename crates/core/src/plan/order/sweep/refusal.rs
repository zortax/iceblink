//! The error of an order class too large to sweep.

use crate::plan::order::DrawOrder;

/// An order class with more instances than the sweep's cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooManyAtOneOrder {
    /// The order of the class.
    pub order: DrawOrder,
    /// How many instances the class holds.
    pub held: usize,
    /// The cap it exceeded.
    pub max: usize,
}

impl core::fmt::Display for TooManyAtOneOrder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "draw order {} holds {} instances, past the cap of {}",
            self.order, self.held, self.max
        )
    }
}

impl core::error::Error for TooManyAtOneOrder {}
