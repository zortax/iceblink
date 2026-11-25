//! The client seam: a compositor run by a driver, reached through queued calls, events and
//! published state.

mod event;
mod factory;
mod status;
mod watch;

use std::sync::Arc;

use crate::smallvec::SmallVec;
use crate::{
    ClientId, CompositorBudgets, CompositorCapabilities, CompositorMemory, FontRegistry,
    FrameClock, HitResult, IdSpace, OutputConfig, OutputId, ScrollGesture, ScrollId, ScrollRouting,
    SubpixelOrder, Transaction,
};

pub use self::event::{ClientEvent, CommitId, WatchCause};
pub use self::factory::{EventWaker, FrameObserver, RendererFactory, RendererTask};
pub use self::status::{DriverPacing, DriverStats, OutputStatus};
pub use self::watch::{OffsetRange, OutputWatch, ScrollWatch};

/// A client's connection to a driver, which runs a compositor and the frame lifecycle of its
/// outputs.
///
/// Calls are queued and applied in the order they were made, between frames. The driver returns
/// their results as events in the client's queue and as state it publishes, so no call except
/// [`register_client`](Self::register_client), [`hit_test`](Self::hit_test),
/// [`create_output`](Self::create_output) and [`sync`](Self::sync) can wait for the driver.
///
/// # Frames
///
/// A frame of an output shows every call queued before the frame latched, and none queued after.
/// When a frame latches depends on the driver's pacing:
///
/// - A paced driver latches each frame on its own, at a deadline before the display time the
///   frame is built for.
/// - A stepped driver latches a frame only at a [`step`](Self::step). A frame a client's call
///   wants is built by the next step, whatever its clock, and the compositor's own motion once
///   per interval of the step clocks.
///
/// A driver that runs on the client's thread builds its frames inside [`pump`](Self::pump) and
/// `step`, and [`pacing`](Self::pacing) says which the client calls. The next frame latches at
/// [`DriverStats::next_due_ns`].
///
/// # Events and wakes
///
/// Each client has its own event queue. A queued `CompositorEvent::Scroll` is replaced by a newer
/// event of the same node, unless it reports `ScrollPhase::Settled`. Whether an event wakes its
/// client depends on its kind:
///
/// - `Rejected`, `Rerouted`, `Watched`, `OutputFailed`, `Shown` and `PresenterChanged`, and the
///   compositor's `HintViolated` and `DamageUndeclared`, wake the client.
/// - The compositor's `Animation` events wake the client while it watches its animations.
/// - `Committed`, `Drawn` and the compositor's `Scroll` events never wake it.
///
/// A client is woken by calling its waker, at most once between two drains of its queue and once
/// more after each change of its waker.
pub trait CompositorClient {
    /// Returns what the compositor supports, with `threaded` set when the driver builds frames on
    /// a thread of its own.
    fn capabilities(&self) -> CompositorCapabilities;

    /// Returns who runs the driver's frames, which decides whether the client steps or pumps it.
    fn pacing(&self) -> DriverPacing;

    /// Returns the registry of the fonts the compositor rasterises glyphs from.
    fn font_registry(&self) -> Arc<dyn FontRegistry>;

    /// Adds a client and returns its id and the id space its transactions mint from.
    ///
    /// The call may wait for the frame in progress only when several clients are registered
    /// between two batches of the driver.
    fn register_client(&mut self) -> (ClientId, IdSpace);

    /// Removes `client`, everything it created, its event queue, its waker and its watches.
    ///
    /// Events of the client queued after the call are dropped.
    fn unregister_client(&mut self, client: ClientId);

    /// Returns an empty transaction for `client`.
    ///
    /// The transaction reuses the buffers of the client's last applied commit once the driver has
    /// returned them.
    ///
    /// # Panics
    ///
    /// Panics if `client` is not registered through this driver.
    fn transaction(&mut self, client: ClientId) -> Transaction;

    /// Queues a commit of `transaction` and returns its id.
    ///
    /// The commit is answered by one `Committed` or one `Rejected` event, in the order commits
    /// are applied. A commit queued behind a rejected one applies on its own.
    fn commit(&mut self, transaction: Transaction) -> CommitId;

    /// Adds an output and returns its id.
    ///
    /// The factory runs on the thread that draws the output's frames, and the renderer it builds
    /// is configured for `config`. The call waits for the driver. A driver that draws on a thread
    /// of its own returns before the factory runs, and the calls queued before the renderer is
    /// built apply once it is. The output's snapshot is published once the renderer is built.
    ///
    /// If the factory fails, the output draws nothing, its snapshot reports it failed, and every
    /// client gets an `OutputFailed` event.
    fn create_output(&mut self, config: OutputConfig, renderer: RendererFactory) -> OutputId;

    /// Changes an output's size, scale or opacity, and configures its renderer to match.
    ///
    /// Once the output's frames have started, the change takes effect with the next commit the
    /// compositor applies or `request_frame` of the output queued after it, and a later change
    /// replaces it. The output's frames keep the configuration before it until then.
    fn configure_output(&mut self, output: OutputId, config: OutputConfig);

    /// Sets the display's stripe order for the subpixel sprites of an output, or `None` to draw
    /// none.
    ///
    /// The order is passed to the output's renderer and to every later frame request.
    fn set_subpixel_order(&mut self, output: OutputId, order: Option<SubpixelOrder>);

    /// Sets the refresh interval of the display an output is shown on, as the platform reports
    /// it, or `None` when the platform reports none.
    ///
    /// A paced driver builds the output's frames at that interval in preference to one it
    /// measured or assumed, unless the output's presenter reports one itself.
    fn set_refresh(&mut self, output: OutputId, refresh_ns: Option<u64>);

    /// Reports that a frame of an output was shown on the display at `presented_ns`, on the
    /// frame clock, as the platform reports it.
    ///
    /// A paced driver takes the reports as the refreshes of the output's display, which its
    /// frames latch on, unless the output's presenter reports when its presents were shown.
    fn report_presented(&mut self, output: OutputId, presented_ns: u64);

    /// Removes an output and drops its renderer.
    fn destroy_output(&mut self, output: OutputId);

    /// Asks for a frame of an output, such as when a window is shown again or a lost device has a
    /// replacement.
    fn request_frame(&mut self, output: OutputId);

    /// Queues a scroll gesture at a device point of `output`, which `client` predicts the
    /// compositor routes as `predicted`.
    ///
    /// If the compositor routes the gesture otherwise, `client` gets a `Rerouted` event. A paced
    /// driver stamps the gesture with the frame clock when the call is made, and a stepped driver
    /// keeps `gesture.time_ns`.
    fn scroll_input(
        &mut self,
        client: ClientId,
        output: OutputId,
        gesture: ScrollGesture,
        predicted: ScrollRouting,
    );

    /// Returns every item under a device point of `output`, front to back.
    ///
    /// The answer reflects every call queued before this one and the scroll offsets and animated
    /// values of the most recent frame. The call waits for the driver, on a driver with a thread
    /// of its own until the frame in progress is drawn and presented.
    fn hit_test(&mut self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]>;

    /// Sets the motion of a scroll node of `client` that wakes the client, or removes it with
    /// `None`.
    ///
    /// The watch replaces the node's earlier one and is checked from the next frame on. A watch
    /// that fires queues a `Watched` event.
    fn watch_scroll(&mut self, client: ClientId, node: ScrollId, watch: Option<ScrollWatch>);

    /// Sets whether the compositor's animation events of `client` wake it.
    fn watch_animations(&mut self, client: ClientId, on: bool);

    /// Sets whether `client` gets a `Drawn` event for every frame drawn for `output`.
    fn watch_frames(&mut self, client: ClientId, output: OutputId, on: bool);

    /// Sets the changes of `output` that wake `client`, replacing its earlier watch of the
    /// output.
    fn watch_output(&mut self, client: ClientId, output: OutputId, watch: OutputWatch);

    /// Sets the function that sees every frame built for `output` before it is drawn, or removes
    /// it with `None`.
    fn observe_frames(&mut self, output: OutputId, observer: Option<FrameObserver>);

    /// Queues work on the renderer of `output`, run between frames on the thread that draws
    /// them.
    ///
    /// The task is dropped without running if the output does not exist or has no renderer.
    fn with_renderer(&mut self, output: OutputId, task: RendererTask);

    /// Queues `Renderer::release_cached_targets` on the renderer of `output`.
    fn release_cached_targets(&mut self, output: OutputId);

    /// Queues `Renderer::release_idle_resources` on the renderer of `output`.
    fn release_idle_resources(&mut self, output: OutputId);

    /// Sets the function that wakes `client`, or removes it with `None`.
    ///
    /// Events queued before the call do not call the new waker.
    fn set_waker(&mut self, client: ClientId, waker: Option<EventWaker>);

    /// Moves the queued events of `client` into `out`, in the order they happened.
    fn drain_events(&mut self, client: ClientId, out: &mut Vec<ClientEvent>);

    /// Returns the newest snapshot of the driver and its compositor.
    fn stats(&self) -> DriverStats;

    /// Returns the memory the compositor holds.
    ///
    /// A threaded driver answers from a copy it renews when it publishes and the copy is 100 ms
    /// old or older, and whenever it answers a `sync`.
    fn memory(&self) -> CompositorMemory;

    /// Returns the newest snapshot of an output, or `None` if the output does not exist.
    fn output_status(&self, output: OutputId) -> Option<OutputStatus>;

    /// Calls `out` with each implementation-specific counter of the compositor, by name, as of
    /// the newest snapshot.
    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64));

    /// Calls `out` with each renderer-specific counter of an output's renderer, by name, as of
    /// the output's newest snapshot.
    fn output_extra_stats(&self, output: OutputId, out: &mut dyn FnMut(&'static str, u64));

    /// Queues an eviction of unpinned atlas entries until the atlas pages hold at most
    /// `to_bytes`.
    fn trim(&mut self, to_bytes: u64);

    /// Queues a drop of every atlas entry and lowered chunk, pinned ones included.
    fn forget_caches(&mut self);

    /// Queues switching the compositor's verification mode on or off, as
    /// `Compositor::set_verification` defines it.
    fn set_verification(&mut self, on: bool);

    /// Queues new soft limits of the compositor's caches, as `Compositor::set_budgets` defines
    /// them.
    fn set_budgets(&mut self, budgets: CompositorBudgets);

    /// Builds the frame of every output that wants one by `clock.now_ns`, at `clock`.
    ///
    /// The frames include every call queued before the step. A paced driver ignores the call.
    fn step(&mut self, clock: FrameClock);

    /// Runs the work that is due on the calling thread.
    ///
    /// A driver that runs on the client's thread builds the frames that are due. A driver with a
    /// thread of its own does nothing.
    fn pump(&mut self);

    /// Waits until the driver has applied every call made before this one, frames of steps
    /// included.
    fn sync(&mut self);
}
