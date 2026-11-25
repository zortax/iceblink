//! Sink refusals.

/// A refusal from a [`TextureSink`](crate::atlas::TextureSink).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the texture sink refused the operation: {reason}")]
pub struct SinkError {
    /// The sink's description of the failure.
    pub reason: String,
}

impl SinkError {
    /// Creates a refusal with `reason`.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}
