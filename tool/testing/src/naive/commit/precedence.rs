//! Which of several broken rules a rejection reports.

use iceblink::{CommitError, RejectReason};

/// Returns the place of `reason` in the order in which a commit reports broken rules, lowest
/// first.
pub(crate) const fn rank(reason: RejectReason) -> u8 {
    match reason {
        RejectReason::Unsupported => 0,
        RejectReason::StaleId => 1,
        RejectReason::UnknownChunk => 2,
        RejectReason::UnknownOutput => 3,
        RejectReason::SpliceOutOfRange => 4,
        RejectReason::PaintKindChanged => 5,
        RejectReason::InvalidWrite => 6,
        RejectReason::InvalidShader => 7,
        RejectReason::InvalidItem => 8,
        RejectReason::InUse => 9,
        RejectReason::OrderConflict => 10,
        RejectReason::Cycle => 11,
        _ => 12,
    }
}

/// The broken rule of the finished state that a rejection reports.
///
/// It is the rule attributed to the earliest command, and among the rules that command breaks, the
/// first in rule order.
#[derive(Debug, Default)]
pub(crate) struct Earliest(Option<(u32, RejectReason)>);

impl Earliest {
    /// Notes that `command` broke the rule of `reason`.
    pub(crate) fn note(&mut self, command: u32, reason: RejectReason) {
        let earlier = match self.0 {
            None => true,
            Some((held, kept)) => (command, rank(reason)) < (held, rank(kept)),
        };
        if earlier {
            self.0 = Some((command, reason));
        }
    }

    /// Returns the error for the reported rule, or `Ok` when no rule was broken.
    pub(crate) fn into_result(self) -> Result<(), CommitError> {
        match self.0 {
            None => Ok(()),
            Some((command, reason)) => Err(CommitError {
                reason,
                command: Some(command),
            }),
        }
    }
}
