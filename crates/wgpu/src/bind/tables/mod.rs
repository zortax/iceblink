//! The renderer's copies of the frame tables and their GPU encodings, kept current by applying
//! each frame's deltas.
//!
//! Only what a delta changed is encoded again, and the dirty slots list what to upload. A changed
//! space resolves the clips that baked its matrix into their records again.
//!
//! Clip links and gradient stops have variable length. A record that grows takes a new range at
//! the end of its table, and the table is rebuilt once abandoned ranges outnumber live ones.

mod clips;
mod dependents;
mod dirty;
mod paints;
mod records;
mod spaces;

use iceblink::{ClipEntry, FramePaint, Matrix4, ParamsBlock, Tables};

use crate::bind::tables::dependents::Dependents;

pub(crate) use self::dirty::{DirtySlots, DirtyTables};
pub(crate) use self::records::{GpuClip, GpuClipLink, GpuPaint, GpuSpatial, GpuStop};

/// The encoded tables, in the order the shaders index them.
#[derive(Clone, Debug, Default)]
pub(crate) struct GpuTables {
    pub(crate) clips: Vec<GpuClip>,
    pub(crate) links: Vec<GpuClipLink>,
    pub(crate) paints: Vec<GpuPaint>,
    pub(crate) stops: Vec<GpuStop>,
    pub(crate) spatial: Vec<GpuSpatial>,
}

/// A mask link whose cell is still to be placed, by its slot in the link table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MaskRequest {
    pub(crate) link: u32,
    /// The atlas page the mask is copied from.
    pub(crate) page: u32,
    /// The mask's texels in the page, as origin and extent.
    pub(crate) rect: [u32; 4],
}

/// The frame tables as the renderer holds them.
#[derive(Debug, Default)]
pub(crate) struct FrameTables {
    spaces: Vec<Matrix4>,
    clips: Vec<ClipEntry>,
    paints: Vec<FramePaint>,
    params: Vec<ParamsBlock>,
    gpu: GpuTables,
    dirty: DirtyTables,
    /// Per clip, its range of the link table, as start and capacity.
    link_ranges: Vec<(u32, u32)>,
    /// The link slots of abandoned ranges.
    link_garbage: u32,
    /// Per paint, how many stops its range holds from the record's `stop_start`.
    stop_capacity: Vec<u32>,
    /// The stop slots of abandoned ranges.
    stop_garbage: u32,
    /// Per space, whether its matrix only translates.
    translating: Vec<bool>,
    deps: Dependents,
    /// Per clip, whether it waits in `queue`.
    queued: Vec<bool>,
    /// The clips to resolve again this frame.
    queue: Vec<u32>,
    /// The mask links resolved since the last `take_mask_requests`.
    masks: Vec<MaskRequest>,
    /// The stops of the paint being encoded.
    scratch: Vec<GpuStop>,
}

impl FrameTables {
    /// Applies a frame's table deltas.
    ///
    /// Solid and side colours are quantised to eight bits per component.
    pub(crate) fn apply(&mut self, tables: &Tables) {
        self.dirty.clear();

        self.apply_spaces(tables);
        self.apply_clips(tables);
        self.resolve_queued();
        self.apply_paints(tables);
        tables.params.apply(&mut self.params);

        self.dirty.finish();
    }

    /// Marks every table for a whole upload, as after the device textures were released.
    pub(crate) fn mark_all_dirty(&mut self) {
        self.dirty.mark_all();
    }

    pub(crate) fn gpu(&self) -> &GpuTables {
        &self.gpu
    }

    pub(crate) fn dirty(&self) -> &DirtyTables {
        &self.dirty
    }

    pub(crate) fn spaces(&self) -> &[Matrix4] {
        &self.spaces
    }

    pub(crate) fn clips(&self) -> &[ClipEntry] {
        &self.clips
    }

    pub(crate) fn paints(&self) -> &[FramePaint] {
        &self.paints
    }

    pub(crate) fn params(&self) -> &[ParamsBlock] {
        &self.params
    }

    /// Returns how many slots this frame encoded again.
    pub(crate) fn prepared(&self) -> u64 {
        self.dirty.clips.count(self.gpu.clips.len())
            + self.dirty.links.count(self.gpu.links.len())
            + self.dirty.paints.count(self.gpu.paints.len())
            + self.dirty.stops.count(self.gpu.stops.len())
            + self.dirty.spatial.count(self.gpu.spatial.len())
    }

    /// Moves the mask links resolved since the last call into `into`.
    pub(crate) fn take_mask_requests(&mut self, into: &mut Vec<MaskRequest>) {
        into.append(&mut self.masks);
    }

    /// Points mask link `link` at `cell` of the colour masks if `color` is set, and of the alpha
    /// masks otherwise.
    ///
    /// The cell is origin and extent, in texels.
    pub(crate) fn place_mask(&mut self, link: u32, cell: [f32; 4], color: bool) {
        let Some(record) = self.gpu.links.get_mut(link as usize) else {
            return;
        };
        record.extra = cell;
        record.kind = if color {
            records::link::COLOR_MASK
        } else {
            records::link::ALPHA_MASK
        };
        self.dirty.links.push(link);
    }
}
