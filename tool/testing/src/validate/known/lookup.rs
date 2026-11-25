//! The three answers to a question about renderer state.

/// What the frames seen so far say about one entry of renderer state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup<T> {
    /// The entry exists and holds this value.
    Found(T),
    /// The entry does not exist.
    Missing,
    /// The frames seen so far do not say.
    Unknown,
}

impl<T> Lookup<T> {
    /// Returns the value, if the entry is known to exist.
    pub(crate) fn found(self) -> Option<T> {
        match self {
            Self::Found(value) => Some(value),
            Self::Missing | Self::Unknown => None,
        }
    }

    /// Reports whether the entry is known not to exist.
    pub(crate) fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}
