//! The identity of an arena.

/// The identity of an arena, carried in every key it issues.
///
/// An arena never resolves a key of another namespace. Every `u16` is a valid namespace.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct NamespaceId(u16);

impl NamespaceId {
    /// The namespace with raw value zero.
    pub const FIRST: Self = Self(0);

    /// Wraps a raw namespace.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the raw namespace.
    pub const fn get(self) -> u16 {
        self.0
    }
}
