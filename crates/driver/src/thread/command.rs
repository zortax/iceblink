//! The calls a client handle queues for the driver thread.

use std::sync::mpsc::Sender;

use iceblink::smallvec::SmallVec;
use iceblink::{
    ClientId, CommitId, CompositorBudgets, FrameClock, FrameObserver, HitResult, IdSpace,
    OutputConfig, OutputId, OutputWatch, RendererFactory, RendererTask, ScrollGesture, ScrollId,
    ScrollRouting, ScrollWatch, SubpixelOrder, Transaction,
};

/// A call to apply on the driver thread, with the channel its answer is sent on.
pub(crate) enum Command {
    /// A handle took a reserve client, so the driver registers another.
    Reserve,
    Register(Sender<(ClientId, IdSpace)>),
    Unregister(ClientId),
    Commit(CommitId, Transaction),
    CreateOutput {
        config: OutputConfig,
        renderer: RendererFactory,
        reply: Sender<OutputId>,
    },
    ConfigureOutput(OutputId, OutputConfig),
    SetSubpixelOrder(OutputId, Option<SubpixelOrder>),
    SetRefresh(OutputId, Option<u64>),
    ReportPresented(OutputId, u64),
    DestroyOutput(OutputId),
    RequestFrame(OutputId),
    ScrollInput {
        client: ClientId,
        output: OutputId,
        gesture: ScrollGesture,
        predicted: ScrollRouting,
    },
    HitTest(OutputId, [f32; 2], Sender<SmallVec<[HitResult; 4]>>),
    WatchScroll(ClientId, ScrollId, Option<ScrollWatch>),
    WatchAnimations(ClientId, bool),
    WatchFrames(ClientId, OutputId, bool),
    WatchOutput(ClientId, OutputId, OutputWatch),
    ObserveFrames(OutputId, Option<FrameObserver>),
    WithRenderer(OutputId, RendererTask),
    ReleaseCachedTargets(OutputId),
    ReleaseIdleResources(OutputId),
    Trim(u64),
    ForgetCaches,
    SetVerification(bool),
    SetBudgets(CompositorBudgets),
    Step(FrameClock),
    Sync(Sender<()>),
    Shutdown,
}
