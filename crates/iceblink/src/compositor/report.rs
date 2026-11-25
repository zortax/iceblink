//! What a successful commit did.

use crate::smallvec::SmallVec;
use crate::{AnimationId, CommitCounters};

/// What a successful commit did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitReport {
    /// The animations the commit started.
    pub animations: SmallVec<[AnimationId; 2]>,
    /// The work the commit did, counted.
    pub counters: CommitCounters,
}
