//! Handles the compositor mints.

/// A client of a compositor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClientId(pub u32);

/// An output of a compositor, minted by the compositor when the output is created.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

/// A frame of one output.
///
/// The frame ids of an output increase by one per built frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(pub u64);
