//! The scene seam: transactions in, frames out.

mod capabilities;
mod memory;
mod output;
mod report;

use std::sync::Arc;

use crate::smallvec::SmallVec;
use crate::{
    Built, ClientId, CommitError, CompositorEvent, Counters, FontRegistry, Frame, FrameId,
    FrameOutcome, FrameRequest, GlyphRaster, HitResult, IdSpace, OutputId, ScrollGesture,
    ScrollRouting, Transaction,
};

pub use self::capabilities::CompositorCapabilities;
pub use self::memory::{CompositorBudgets, CompositorMemory};
pub use self::output::OutputConfig;
pub use self::report::CommitReport;

/// A retained compositor: it applies clients' transactions and builds frames for its outputs.
///
/// # Frames
///
/// A new output has no root and draws nothing. Every frame built for an output is passed to that
/// output's renderer before the next frame is built for it, and its outcome is reported with
/// `frame_done`. Each frame's table deltas, chunk notes and resource work are sent once.
///
/// # Budgets
///
/// The atlas holds rasterised glyphs, atlased images and mark shapes. An atlas entry is pinned at
/// least while a lowered chunk references it that an output's renderer holds resident, that the
/// last frame of an output drew, or that the frame being built draws. Eviction removes only
/// unpinned entries and destroys the pages it empties. A lowered chunk that references an evicted
/// entry is lowered again before it is drawn.
///
/// # The compositor-driven tier
///
/// A compositor reports animations, scroll input and hit testing in its capabilities. Without
/// them, it rejects transactions that animate with `RejectReason::Unsupported`, commits a cancel
/// or a removal of an animation id as doing nothing, `scroll_input` returns
/// `ScrollRouting::Unhandled`, and `hit_test` returns no hit.
pub trait Compositor: Send {
    /// Returns what the compositor supports.
    fn capabilities(&self) -> CompositorCapabilities;

    /// Adds a client and returns its id and the id space its transactions mint from.
    fn register_client(&mut self) -> (ClientId, IdSpace);

    /// Removes `client` and everything it created.
    fn unregister_client(&mut self, client: ClientId);

    /// Returns an empty transaction for `client`, with buffers recycled from earlier commits.
    ///
    /// The commit of a transaction for a client that is not registered is rejected with
    /// `RejectReason::StaleId`.
    fn transaction(&mut self, client: ClientId) -> Transaction;

    /// Applies `transaction` atomically.
    ///
    /// Either every command applies, or none does and the ids the transaction minted are freed.
    ///
    /// # Errors
    ///
    /// Returns a rule the transaction broke and the command that broke it. When several rules are
    /// broken, the error's command is the first command that breaks a rule as it applies, or else
    /// the earliest command that a rule of the finished state is attributed to.
    ///
    /// A transaction for a client that is not registered, or built on another id space, is
    /// rejected with `RejectReason::StaleId` and no command.
    fn commit(&mut self, transaction: Transaction) -> Result<CommitReport, CommitError>;

    /// Shares the client's font registry, so a font has the same id in the client's layout and in
    /// the compositor's glyph cache.
    ///
    /// While any chunk exists, the call drops the caches as `forget_caches` does.
    fn use_font_registry(&mut self, registry: Arc<dyn FontRegistry>);

    /// Returns the font registry in use, the compositor's own until `use_font_registry` replaces
    /// it.
    fn font_registry(&self) -> Arc<dyn FontRegistry>;

    /// Sets the rasteriser glyphs are rasterised through.
    ///
    /// While any chunk exists, the call drops the caches as `forget_caches` does.
    fn use_glyph_raster(&mut self, raster: Arc<dyn GlyphRaster>);

    /// Adds an output and returns its id.
    fn create_output(&mut self, config: OutputConfig) -> OutputId;

    /// Changes an output's size, scale or opacity.
    fn configure_output(&mut self, output: OutputId, config: OutputConfig);

    /// Removes an output and its history.
    ///
    /// A later output with a new renderer starts with full damage and full tables.
    fn destroy_output(&mut self, output: OutputId);

    /// Builds the next frame of `output` into `into`, keeping its allocations.
    ///
    /// Returns `Built::Idle` and leaves `into` untouched only when the frame would have no damage
    /// and no shift. Resource work alone needs no frame and may wait for the output's next frame.
    ///
    /// A frame with no damage may still be built only to deliver chunk notes, table deltas and
    /// resource work, such as for an idle output that other outputs left far behind.
    fn build_frame(
        &mut self,
        output: OutputId,
        request: &FrameRequest<'_>,
        into: &mut Frame,
    ) -> Built;

    /// Reports what drawing frame `frame` of `output` did.
    ///
    /// The outcome decides whether the frame's damage retires. After `Recovered` or
    /// `Skipped(DeviceUnavailable)`, the next frame of the output is built as the first one for a
    /// new renderer.
    fn frame_done(&mut self, output: OutputId, frame: FrameId, outcome: &FrameOutcome);

    /// Returns when `output` next changes without a call, as of the frame last built for it.
    ///
    /// The answer is the present time of the frame that shows the change, in the same terms as
    /// `Built::Idle::wake_ns`, and a time already past asks for the next frame. Running
    /// animations, scroll motion, content that reads the clock and frame paths that keep their
    /// route history ask for the next frame. `None` means nothing changes until a call. The
    /// default asks for the next frame.
    fn next_wake(&self, output: OutputId) -> Option<u64> {
        let _ = output;
        Some(0)
    }

    /// Routes a scroll gesture at a device point of `output` and moves the scroll nodes that take
    /// its delta.
    fn scroll_input(&mut self, output: OutputId, gesture: ScrollGesture) -> ScrollRouting;

    /// Returns every item under a device point of `output`, front to back.
    fn hit_test(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]>;

    /// Moves the queued events into `out`, in the order they happened.
    fn drain_events(&mut self, out: &mut Vec<CompositorEvent>);

    /// Switches verification mode on or off.
    ///
    /// While it is on, hints and declared damage do not narrow the damage of a commit. A
    /// compositor that reports `precise` also reports every hint and declaration that falls
    /// short, through events and counters.
    fn set_verification(&mut self, on: bool);

    /// Returns the counters of the most recent commit and frame.
    fn counters(&self) -> &Counters;

    /// Returns the memory the compositor holds.
    fn memory(&self) -> CompositorMemory;

    /// Sets the soft limits of the compositor's caches.
    ///
    /// When a frame is built while the atlas is over its limit, unpinned entries are evicted
    /// until it is under the limit or no unpinned entry is left.
    fn set_budgets(&mut self, budgets: CompositorBudgets);

    /// Returns the soft limits in use, which have no limit until `set_budgets` is called.
    fn budgets(&self) -> CompositorBudgets;

    /// Evicts unpinned atlas entries until the atlas pages hold at most `to_bytes`, and returns
    /// the bytes of the pages destroyed.
    ///
    /// `memory` reflects the eviction when this returns. The page destructions are sent with the
    /// next frame of each output.
    fn trim(&mut self, to_bytes: u64) -> u64;

    /// Drops every atlas entry and lowered chunk, pinned ones included, and returns the bytes of
    /// the pages destroyed.
    ///
    /// `memory` then reports no atlas bytes. The next frame of each output destroys every page its
    /// renderer holds, resets its resident chunks, rasterises and inserts again what it draws, and
    /// damages the whole output.
    fn forget_caches(&mut self) -> u64;

    /// Calls `out` with each implementation-specific counter, by name.
    ///
    /// The default reports none.
    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        let _ = out;
    }
}
