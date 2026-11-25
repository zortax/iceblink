//! The frame: the finished, renderer-neutral plan for one output image.
//!
//! The compositor decides order, culling, batching, group nesting, vector passes, mark bins,
//! shifts and opaque covers, and a renderer executes that plan. The frame is expressed in frame
//! kinds, a renderer-facing set of instance layouts that client items are lowered into.

mod batch;
mod clock;
mod cover;
mod damage;
mod kind;
mod lane;
mod layout;
mod mark_plan;
mod request;
mod resident;
mod resource;
mod shift;
mod table;
mod vector;

use smallvec::SmallVec;

use crate::id::{FrameId, OutputId};

pub use self::batch::{Batch, BatchKind};
pub use self::clock::{FrameClock, ShaderClock};
pub use self::cover::OpaqueCover;
pub use self::damage::FrameDamage;
pub use self::kind::{
    AtlasTile, ColorSprite, FrameBackdrop, FrameExternal, FrameFilter, FrameGroup,
    FrameMarkPayload, FrameMarks, FramePath, FrameShadow, Instance, MarkCell, Sprite, SpriteFlags,
};
pub use self::lane::{FRAME_NO_CLIP, ItemSource, Lane, Lanes};
pub use self::mark_plan::{MAX_MARK_PAGES, MarkBin, MarkPlan};
pub use self::request::{Built, FrameRequest};
pub use self::resident::{ChunkNote, DrawnChunk, LoweredChunk};
pub use self::resource::{PageFormat, PageOp, ResourceWork, ShaderOp};
pub use self::shift::Shift;
pub use self::table::{
    ClipEntry, ClipLinkEntry, FrameClipShape, FrameGradient, FramePaint, FrameStop, StopEncoding,
    TableDelta, Tables,
};
pub use self::vector::{PassKey, PlannedPass, PlannedPath, VectorPlan};

/// The finished plan for one output image.
///
/// A frame is owned and `Send`, so it can be built on one thread and drawn on another. Building
/// into a frame keeps its allocations.
///
/// A renderer draws a frame in this sequence:
///
/// 1. It applies `resources`, then `chunk_notes`, then the table deltas.
/// 2. It applies `shifts` to the kept target.
/// 3. It redraws `damage.draw`: it clears each draw rectangle unless an opaque cover lets it skip
///    the clear, then draws the batches in sequence, cut to the rectangle.
///
/// Every source, batch range, index and frame slot is valid once the frame's notes, table deltas
/// and resource work are applied.
#[derive(Debug, Default)]
pub struct Frame {
    /// The frame's id, one more than the previous frame of the output.
    pub id: FrameId,
    /// The output the frame is for.
    pub output: OutputId,
    /// The output's size, in device pixels.
    pub size: [u32; 2],
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// Whether the output's final image is opaque.
    pub opaque: bool,
    /// The clock the frame was requested with.
    pub clock: FrameClock,
    /// The times application shaders read.
    pub shader_clock: ShaderClock,
    /// The pixels redrawn and the pixels presented as changed.
    pub damage: FrameDamage,
    /// Moves of kept pixels, applied before anything is drawn.
    pub shifts: SmallVec<[Shift; 1]>,
    /// Changes to the resident chunks, applied in order.
    pub chunk_notes: Vec<ChunkNote>,
    /// The resident chunks drawn, which resident sources index.
    pub drawn: Vec<DrawnChunk>,
    /// The instanced frame kinds.
    pub lanes: Lanes,
    /// The paths the vector plan rasterises.
    pub paths: Vec<FramePath>,
    /// The external quads `External` batches draw.
    pub externals: Vec<FrameExternal>,
    /// The backdrops `Backdrop` batches apply.
    pub backdrops: Vec<FrameBackdrop>,
    /// The groups `GroupBegin` and `GroupEnd` batches open and composite.
    pub groups: Vec<FrameGroup>,
    /// The draw sequence.
    pub batches: Vec<Batch>,
    /// The vector passes.
    pub vector_plan: VectorPlan,
    /// The bins of union marks.
    pub mark_plan: MarkPlan,
    /// Quads that let a renderer skip what lies beneath them, at most four.
    pub opaque_covers: SmallVec<[OpaqueCover; 4]>,
    /// The table deltas.
    pub tables: Tables,
    /// The atlas page and shader operations.
    pub resources: ResourceWork,
}

impl Frame {
    /// Empties every list, every table delta and every damage set, keeping the allocations.
    ///
    /// The id, output, size, scale, opacity and clocks are kept.
    pub fn clear(&mut self) {
        self.damage = FrameDamage::default();
        self.shifts.clear();
        self.chunk_notes.clear();
        self.drawn.clear();
        self.lanes.clear();
        self.paths.clear();
        self.externals.clear();
        self.backdrops.clear();
        self.groups.clear();
        self.batches.clear();
        self.vector_plan.clear();
        self.mark_plan.clear();
        self.opaque_covers.clear();
        self.tables.clear();
        self.resources.clear();
    }
}
