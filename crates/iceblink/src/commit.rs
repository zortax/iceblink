//! Why a commit is rejected.

use core::fmt;

/// A rejected commit: the rule it broke and, when one command broke it, that command.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitError {
    /// The rule the transaction broke.
    pub reason: RejectReason,
    /// The index of the command that broke the rule, or `None` when the transaction's client
    /// or id space is wrong or the compositor stopped before applying it.
    ///
    /// When a transaction breaks several rules, this is the first command that breaks a rule as
    /// it applies, or else the earliest command that a rule of the finished state is attributed
    /// to.
    pub command: Option<u32>,
}

/// The rule a rejected transaction broke.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    /// An id was removed or has a stale generation, a create reuses a live slot, or the
    /// transaction was built on a foreign id space.
    StaleId,
    /// A removed object is still referred to by a chunk, a node, a slot or an order entry.
    InUse,
    /// A transform, clip or order parent chain would form a cycle.
    Cycle,
    /// An order node or chunk key would appear in two places.
    OrderConflict,
    /// A splice's range ends past the end of a sequence.
    SpliceOutOfRange,
    /// `set_paint` would change a slot's kind, or a `PaintRef` refers to a paint of another kind.
    PaintKindChanged,
    /// An item, a node or a paint holds a non-finite number, an invalid colour or an unlisted
    /// enumeration value, or an item holds a slot of no live object of its kind or a range past
    /// its arena.
    ///
    /// The commit also returns it for a malformed command value, such as a non-finite chunk
    /// offset, a local clip parent that is not an earlier local clip, or an unlisted image format.
    InvalidItem,
    /// `set_offset`, `set_props` or `remove_chunk` refers to a chunk that does not exist.
    UnknownChunk,
    /// The output does not exist.
    UnknownOutput,
    /// An image write lies outside the image or does not match its format.
    InvalidWrite,
    /// A shader's mode or reads are unlisted or its parameters are larger than 64 bytes, or a
    /// shader is used in the wrong mode.
    InvalidShader,
    /// The transaction animates, and the compositor does not implement the animation tier.
    Unsupported,
    /// The compositor stopped before it applied the transaction.
    Stopped,
}

impl fmt::Display for CommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "commit rejected: {:?}", self.reason)?;
        if let Some(command) = self.command {
            write!(f, " at command {command}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CommitError {}
