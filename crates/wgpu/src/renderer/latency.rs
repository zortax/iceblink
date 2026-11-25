//! Marks at the steps of a frame, for an application's latency traces.

use std::sync::Arc;

/// A function called with the name of each step of a frame as it is reached, and a detail for
/// some.
pub type LatencyMarks = Arc<dyn Fn(&'static str, Option<&str>) + Send + Sync>;

/// The latency marks a renderer emits, or none.
#[derive(Clone, Default)]
pub(crate) struct Latency(Option<LatencyMarks>);

impl Latency {
    /// Returns the latency of a renderer that emits `marks`, if any.
    pub(crate) fn new(marks: Option<LatencyMarks>) -> Self {
        Self(marks)
    }

    /// Marks the step `name`.
    pub(crate) fn mark(&self, name: &'static str) {
        if let Some(marks) = &self.0 {
            marks(name, None);
        }
    }

    /// Marks the step `name` with the text `detail` returns, which is made only when marks are on.
    pub(crate) fn note(&self, name: &'static str, detail: impl FnOnce() -> String) {
        if let Some(marks) = &self.0 {
            marks(name, Some(&detail()));
        }
    }
}

impl std::fmt::Debug for Latency {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(if self.0.is_some() {
            "Latency(on)"
        } else {
            "Latency(off)"
        })
    }
}
