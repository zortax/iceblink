//! What a driver tells one client.

use crate::{
    CommitError, CommitReport, CompositorEvent, FrameCounters, FrameId, FrameOutcome,
    GpuUnavailable, OutputId, ScrollGesture, ScrollId, ScrollRouting,
};

/// The number a driver assigns to a commit when it is queued.
///
/// A commit id is unique among the commits queued through one driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitId(pub u64);

/// Something a driver queued for one client, in the order it happened.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ClientEvent {
    /// An event of the compositor about the client.
    Compositor(CompositorEvent),
    /// A commit was applied.
    Committed {
        /// The id the commit was queued under.
        commit: CommitId,
        /// What the commit did.
        report: CommitReport,
    },
    /// A commit was rejected, and none of its changes applied.
    Rejected {
        /// The id the commit was queued under.
        commit: CommitId,
        /// The rule the transaction broke.
        error: CommitError,
    },
    /// A gesture was routed elsewhere than the client predicted.
    Rerouted {
        /// The output the gesture was handed to.
        output: OutputId,
        /// The gesture the client queued.
        gesture: ScrollGesture,
        /// The routing the client predicted.
        predicted: ScrollRouting,
        /// The routing the compositor chose.
        routing: ScrollRouting,
    },
    /// A scroll watch of the client fired.
    Watched {
        /// The scroll node the watch is set on.
        node: ScrollId,
        /// What fired.
        cause: WatchCause,
        /// The offset the node shows.
        offset: [f32; 2],
        /// The serial of the scroll event that fired the watch.
        serial: u64,
        /// The node's motion step: how far its shown offset moved between its last two scroll
        /// events.
        step: [f32; 2],
    },
    /// The renderer of an output could not be built, so the output draws nothing.
    OutputFailed {
        /// The output whose renderer failed.
        output: OutputId,
        /// Why no device could draw it.
        error: GpuUnavailable,
    },
    /// An output the client watches presented its first frame since the watch was set.
    Shown {
        /// The watched output.
        output: OutputId,
        /// The first frame the output presented since the watch was set.
        frame: FrameId,
    },
    /// The presenter state of an output the client watches changed.
    PresenterChanged {
        /// The watched output.
        output: OutputId,
        /// Whether the presenter's frames time out waiting for an image.
        starved: bool,
        /// Whether the platform reports the output hidden.
        occluded: bool,
    },
    /// A frame of an output the client watches was drawn.
    Drawn {
        /// The output whose frame was drawn.
        output: OutputId,
        /// The drawn frame.
        frame: FrameId,
        /// The counters of the frame.
        counters: FrameCounters,
        /// What drawing the frame did.
        outcome: FrameOutcome,
    },
}

/// What fired a scroll watch.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchCause {
    /// The offset left one of the watch's ranges, given by its index.
    Left(usize),
    /// The offset crossed a multiple of the watch's stride.
    Crossed,
    /// The node came to rest.
    Settled,
}
