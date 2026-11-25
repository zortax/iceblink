//! The failure to find a usable graphics device.

use core::fmt;

/// No usable graphics device was found.
///
/// Every adapter that was considered is listed with the reason it was rejected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GpuUnavailable {
    /// The adapters considered, in the order they were considered.
    pub candidates: Vec<RejectedAdapter>,
}

impl GpuUnavailable {
    /// Returns the failure with `name` added as rejected for `reason`.
    #[must_use]
    pub fn rejected(mut self, name: impl Into<String>, reason: impl Into<String>) -> Self {
        self.candidates.push(RejectedAdapter {
            name: name.into(),
            reason: reason.into(),
        });
        self
    }
}

/// An adapter that was considered and rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedAdapter {
    /// The name the adapter reports.
    pub name: String,
    /// Why the adapter could not be used.
    pub reason: String,
}

impl fmt::Display for GpuUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no usable graphics device: {} adapter(s) were tried and rejected",
            self.candidates.len()
        )?;
        for candidate in &self.candidates {
            write!(f, "; {}: {}", candidate.name, candidate.reason)?;
        }
        Ok(())
    }
}

impl std::error::Error for GpuUnavailable {}
