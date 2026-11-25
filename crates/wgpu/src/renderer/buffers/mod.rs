//! The buffers and tables a frame stages its data through, and the bind groups over them.
//!
//! Bind groups are kept and made again only when a buffer they read was replaced.

mod binds;
mod view_binds;

use std::cell::RefCell;

use bytemuck::Pod;
use iceblink::{Frame, ParamsBlock};

use crate::bind::globals::Globals;
use crate::bind::masks::ClipMasks;
use crate::bind::tables::{DirtySlots, FrameTables};
use crate::buffer::chunks::{ChunkStore, LANES, MARKS_LANE, OrderEntry, PAYLOAD_LANES};
use crate::buffer::instances::{InstanceTable, OrderBuffer};
use crate::buffer::slots::SlotBuffer;
use crate::buffer::vectors::VectorInstances;
use crate::gpu::device::Gpu;
use crate::pipeline::composite::CompositeParams;
use crate::pipeline::kind::PipelineKind;
use crate::pipeline::marks::MarkDraw;
use crate::upload::{TableTexture, UploadBelt};

use self::view_binds::ViewBinds;

/// The buffers a renderer stages its frames through.
#[derive(Debug)]
pub(crate) struct FrameBuffers {
    pub(crate) tables: FrameTables,
    pub(crate) masks: ClipMasks,
    pub(crate) uploader: UploadBelt,
    /// The belt's chunk allocations already added to the renderer's counters.
    pub(crate) allocations_counted: u64,
    pub(crate) globals: SlotBuffer,
    pub(crate) blocks: SlotBuffer,
    pub(crate) effect_params: SlotBuffer,
    pub(crate) mark_draws: SlotBuffer,
    pub(crate) effect_offsets: Vec<u32>,
    pub(crate) vectors: VectorInstances,
    pub(crate) clips: TableTexture,
    pub(crate) links: TableTexture,
    pub(crate) paints: TableTexture,
    pub(crate) stops: TableTexture,
    pub(crate) spatial: TableTexture,
    pub(crate) chunks: ChunkStore,
    pub(crate) orders: [OrderBuffer; LANES],
    /// The draw order of the mark lane.
    mark_order: InstanceTable,
    /// The draw order each lane uploaded last.
    last_orders: [Vec<OrderEntry>; LANES],
    frame_bind: RefCell<Option<([u64; 7], wgpu::BindGroup)>>,
    instance_binds: RefCell<[binds::Keyed<2>; LANES]>,
    mark_binds: RefCell<[binds::Keyed<3>; PAYLOAD_LANES]>,
    filtered_binds: RefCell<ViewBinds>,
    vector_binds: RefCell<ViewBinds>,
    effect_bind: RefCell<Option<(u64, wgpu::BindGroup)>>,
    mark_composite_bind: RefCell<Option<([u64; 2], wgpu::BindGroup)>>,
    /// Whether the table textures were shrunk, so the next upload writes them whole.
    tables_released: bool,
}

impl FrameBuffers {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        Self {
            tables: FrameTables::default(),
            masks: ClipMasks::new(gpu),
            uploader: UploadBelt::default(),
            allocations_counted: 0,
            globals: SlotBuffer::new::<Globals>(gpu, "iceblink.globals"),
            blocks: SlotBuffer::new::<CompositeParams>(gpu, "iceblink.blocks"),
            effect_params: SlotBuffer::with_stride(
                gpu,
                "iceblink.effect.params",
                size_of::<ParamsBlock>() as u64,
            ),
            effect_offsets: Vec::new(),
            mark_draws: SlotBuffer::new::<MarkDraw>(gpu, "iceblink.mark_draws"),
            vectors: VectorInstances::new(gpu),
            clips: TableTexture::new(gpu.device(), "iceblink.clips"),
            links: TableTexture::new(gpu.device(), "iceblink.clip_links"),
            paints: TableTexture::new(gpu.device(), "iceblink.paints"),
            stops: TableTexture::new(gpu.device(), "iceblink.stops"),
            spatial: TableTexture::new(gpu.device(), "iceblink.spatial"),
            chunks: ChunkStore::new(gpu),
            orders: [
                OrderBuffer::new(gpu, "iceblink.order.quads"),
                OrderBuffer::new(gpu, "iceblink.order.shadows"),
                OrderBuffer::new(gpu, "iceblink.order.lines"),
                OrderBuffer::new(gpu, "iceblink.order.mono"),
                OrderBuffer::new(gpu, "iceblink.order.subpixel"),
                OrderBuffer::new(gpu, "iceblink.order.color"),
                OrderBuffer::new(gpu, "iceblink.order.shader_quads"),
                OrderBuffer::new(gpu, "iceblink.order.marks"),
            ],
            mark_order: InstanceTable::new(gpu, "iceblink.mark_order"),
            last_orders: Default::default(),
            frame_bind: RefCell::new(None),
            instance_binds: RefCell::default(),
            mark_binds: RefCell::default(),
            filtered_binds: RefCell::default(),
            vector_binds: RefCell::default(),
            effect_bind: RefCell::new(None),
            mark_composite_bind: RefCell::new(None),
            tables_released: false,
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.uploader.begin_frame();
        self.globals.reset();
        self.blocks.reset();
        self.effect_params.reset();
        self.effect_offsets.clear();
        self.mark_draws.reset();
        self.vectors.begin_frame();
        self.filtered_binds.get_mut().begin_frame();
        self.vector_binds.get_mut().begin_frame();
    }

    /// Uploads the frame tables the frame changed, and returns the bytes written.
    ///
    /// Every table is written whole after the tables were released.
    pub(crate) fn upload_tables(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) -> u64 {
        let tables = self.tables.gpu();
        let dirty = self.tables.dirty();
        let belt = &mut self.uploader;
        let mut uploaded = 0;
        if self.tables_released {
            self.tables_released = false;
            uploaded += gpu.uploaded(
                self.clips
                    .upload(gpu.device(), belt, encoder, &tables.clips),
            );
            uploaded += gpu.uploaded(
                self.links
                    .upload(gpu.device(), belt, encoder, &tables.links),
            );
            uploaded +=
                gpu.uploaded(
                    self.paints
                        .upload(gpu.device(), belt, encoder, &tables.paints),
                );
            uploaded += gpu.uploaded(
                self.stops
                    .upload(gpu.device(), belt, encoder, &tables.stops),
            );
            uploaded +=
                gpu.uploaded(
                    self.spatial
                        .upload(gpu.device(), belt, encoder, &tables.spatial),
                );
            return uploaded;
        }
        uploaded += upload_dirty(
            gpu,
            belt,
            encoder,
            &mut self.clips,
            &tables.clips,
            &dirty.clips,
        );
        uploaded += upload_dirty(
            gpu,
            belt,
            encoder,
            &mut self.links,
            &tables.links,
            &dirty.links,
        );
        uploaded += upload_dirty(
            gpu,
            belt,
            encoder,
            &mut self.paints,
            &tables.paints,
            &dirty.paints,
        );
        uploaded += upload_dirty(
            gpu,
            belt,
            encoder,
            &mut self.stops,
            &tables.stops,
            &dirty.stops,
        );
        uploaded += upload_dirty(
            gpu,
            belt,
            encoder,
            &mut self.spatial,
            &tables.spatial,
            &dirty.spatial,
        );
        uploaded
    }

    /// Uploads every lane's draw order that differs from the last frame's, and returns the bytes
    /// written.
    pub(crate) fn upload_orders(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) -> u64 {
        let mut uploaded = 0;
        for (lane, buffer) in self.orders.iter_mut().enumerate() {
            let resolved = self.chunks.resolved_order(lane);
            // A frame whose drawn set held still resolves to the same order and uploads nothing.
            if resolved == self.last_orders[lane].as_slice() {
                continue;
            }
            uploaded += if lane == MARKS_LANE {
                self.mark_order
                    .upload(gpu, &mut self.uploader, encoder, resolved)
            } else {
                buffer.upload(gpu, &mut self.uploader, encoder, resolved)
            };
            self.last_orders[lane].clear();
            self.last_orders[lane].extend_from_slice(resolved);
        }
        uploaded
    }

    /// Uploads the blocks the plan staged, and returns the bytes written.
    pub(crate) fn upload_blocks(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) -> u64 {
        let mut uploaded = self.globals.upload_with(gpu, &mut self.uploader, encoder);
        uploaded += self.blocks.upload_with(gpu, &mut self.uploader, encoder);
        uploaded += self
            .effect_params
            .upload_with(gpu, &mut self.uploader, encoder);
        uploaded += self
            .mark_draws
            .upload_with(gpu, &mut self.uploader, encoder);
        uploaded += self.vectors.upload_with(gpu, &mut self.uploader, encoder);
        uploaded
    }

    /// Applies the frame's chunk notes and resolves its lanes into draw orders.
    pub(crate) fn resolve_chunks(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
    ) {
        self.chunks
            .apply_notes(gpu, &mut self.uploader, encoder, &frame.chunk_notes);
        self.chunks.resolve(gpu, &mut self.uploader, encoder, frame);
    }

    /// Stages every parameter block of the tables, so each slot has an offset this frame.
    pub(crate) fn stage_effect_params(&mut self) {
        self.effect_offsets.clear();
        for params in self.tables.params() {
            let offset = self.effect_params.stage(params);
            self.effect_offsets.push(offset);
        }
    }

    /// Returns the offset of parameter block `params` staged this frame.
    pub(crate) fn effect_offset(&self, params: u32) -> Option<u32> {
        self.effect_offsets.get(params as usize).copied()
    }

    /// Returns the lane an instanced pipeline draws from.
    pub(crate) fn lane(kind: PipelineKind) -> Option<usize> {
        Some(match kind {
            PipelineKind::Quad => 0,
            PipelineKind::Shadow => 1,
            PipelineKind::Line => 2,
            PipelineKind::MonoSprite => 3,
            PipelineKind::SubpixelSprite => 4,
            PipelineKind::ColorSprite => 5,
            _ => return None,
        })
    }

    /// Returns the bytes of device and staging memory the buffers hold.
    pub(crate) fn bytes(&self) -> u64 {
        self.globals.bytes()
            + self.blocks.bytes()
            + self.mark_draws.bytes()
            + self.effect_params.bytes()
            + self.vectors.bytes()
            + self.clips.capacity()
            + self.links.capacity()
            + self.paints.capacity()
            + self.stops.capacity()
            + self.spatial.capacity()
            + self.chunks.bytes()
            + self.orders.iter().map(OrderBuffer::capacity).sum::<u64>()
            + self.mark_order.capacity()
            + self.uploader.bytes()
    }

    /// Shrinks every buffer and table, forgets the bind groups over them, and returns the bytes
    /// freed.
    pub(crate) fn release_idle(&mut self, gpu: &Gpu) -> u64 {
        let mut freed = self.globals.release() + self.blocks.release() + self.mark_draws.release();
        freed += self.vectors.shrink(gpu);
        freed += self.clips.shrink(gpu.device());
        freed += self.links.shrink(gpu.device());
        freed += self.paints.shrink(gpu.device());
        freed += self.stops.shrink(gpu.device());
        freed += self.spatial.shrink(gpu.device());
        self.tables_released = true;
        self.tables.mark_all_dirty();
        freed += self.chunks.release(gpu);
        for (lane, order) in self.orders.iter_mut().enumerate() {
            freed += order.shrink(gpu);
            self.last_orders[lane].clear();
        }
        // The mark lane's order is a table of its own, and it is released with the others.
        freed += self.mark_order.shrink(gpu);
        freed += self.uploader.release_idle();
        *self.frame_bind.borrow_mut() = None;
        *self.instance_binds.borrow_mut() = Default::default();
        *self.mark_binds.borrow_mut() = Default::default();
        *self.mark_composite_bind.borrow_mut() = None;
        self.forget_view_binds();
        *self.effect_bind.get_mut() = None;
        freed
    }

    /// Forgets the bind groups kept over texture views.
    pub(crate) fn forget_view_binds(&mut self) {
        self.filtered_binds.get_mut().clear();
        self.vector_binds.get_mut().clear();
    }
}

/// Uploads the `dirty` slots of `values` into `table`, and returns the bytes written.
///
/// The slots go up as runs of consecutive slots. The whole table goes up instead when everything
/// is dirty, when at least half the slots are, or when the runs are more than sixteen.
fn upload_dirty<T: Pod>(
    gpu: &Gpu,
    belt: &mut UploadBelt,
    encoder: &mut wgpu::CommandEncoder,
    table: &mut TableTexture,
    values: &[T],
    dirty: &DirtySlots,
) -> u64 {
    const MAX_RANGES: usize = 16;
    let ranges = dirty
        .slots
        .windows(2)
        .filter(|pair| pair[1] != pair[0] + 1)
        .count()
        + usize::from(!dirty.slots.is_empty());
    if dirty.all || dirty.slots.len().saturating_mul(2) >= values.len() || ranges > MAX_RANGES {
        return gpu.uploaded(table.upload(gpu.device(), belt, encoder, values));
    }
    let mut uploaded = 0;
    let mut slots = dirty.slots.iter().copied().peekable();
    while let Some(first) = slots.next() {
        let mut end = first + 1;
        while slots.next_if_eq(&end).is_some() {
            end += 1;
        }
        let range = first as usize..end as usize;
        uploaded += gpu.uploaded(table.upload_range(gpu.device(), belt, encoder, values, range));
    }
    uploaded
}
