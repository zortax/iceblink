//! The state of one frame while it is lowered, and the frame slots it hands out.

mod clips;
mod shading;

use std::collections::{HashMap, HashSet};

use iceblink::{
    ClientId, ClipEntry, DeviceRect, Frame, FrameCounters, FramePaint, Matrix4, ParamsBlock,
    RenderCapabilities, SubpixelOrder,
};
use iceblink::{FontRegistry, GlyphRaster, OutputConfig};

use crate::naive::content::{Content, ImagePage};
use crate::naive::frame::NO_PARAMS;
use crate::naive::frame::chain::Link;
use crate::naive::frame::shaders::ShaderSlots;
use crate::naive::frame::slots::{OutputSlots, TableValues};
use crate::naive::geometry::Bounds;
use crate::naive::scene::Client;
use crate::naive::space::Worlds;

/// The inputs of lowering that stay fixed for the whole frame.
pub(crate) struct Env<'a> {
    pub(crate) client_id: ClientId,
    pub(crate) client: &'a Client,
    pub(crate) raster: Option<&'a dyn GlyphRaster>,
    pub(crate) fonts: &'a dyn FontRegistry,
    pub(crate) caps: RenderCapabilities,
    pub(crate) subpixel: Option<SubpixelOrder>,
    pub(crate) config: OutputConfig,
    /// The output, as a box in device pixels.
    pub(crate) output: Bounds,
}

/// A frame being lowered.
pub(crate) struct Builder<'a> {
    pub(crate) frame: &'a mut Frame,
    pub(crate) env: Env<'a>,
    pub(crate) worlds: Worlds<'a>,
    pub(crate) slots: &'a mut OutputSlots,
    pub(crate) shaders: &'a mut ShaderSlots,
    pub(crate) content: &'a mut Content,
    pub(crate) spaces: TableValues<Matrix4>,
    pub(crate) clips: TableValues<ClipEntry>,
    pub(crate) paints: TableValues<FramePaint>,
    pub(crate) params: TableValues<ParamsBlock>,
    /// The clips of the effects that do not isolate, between the innermost open group and the
    /// content being lowered.
    pub(crate) effect_clips: Vec<Link>,
    /// How many groups are open.
    pub(crate) groups_open: u32,
    /// How far past the output the filters of the open groups read, in device pixels.
    pub(crate) reach: f32,
    /// The index in `Lanes::mark_payloads` of each payload the frame draws, by the addresses of
    /// the payload and its cells.
    pub(crate) mark_payloads: HashMap<(usize, usize), u32>,
    /// The pages the frame draws from.
    pub(crate) pages: HashSet<u32>,
    pub(crate) counters: FrameCounters,
    /// Whether the frame draws with a shader that reads the clock.
    pub(crate) reads_time: bool,
    /// The item being lowered, counted from the frame's first.
    pub(crate) item: u64,
    /// The item the last instance batch drew.
    pub(crate) batched_item: u64,
    /// The position in the marks lane and the device ink of every union marks instance.
    pub(crate) union_marks: Vec<(u32, DeviceRect)>,
}

impl<'a> Builder<'a> {
    /// Returns a builder that lowers the client of `env` into `frame`, with empty tables.
    pub(crate) fn new(
        frame: &'a mut Frame,
        env: Env<'a>,
        slots: &'a mut OutputSlots,
        shaders: &'a mut ShaderSlots,
        content: &'a mut Content,
    ) -> Self {
        Self {
            frame,
            worlds: Worlds::new(env.client),
            env,
            slots,
            shaders,
            content,
            spaces: TableValues::new(Matrix4::IDENTITY),
            clips: TableValues::new(ClipEntry::default()),
            paints: TableValues::new(FramePaint::None),
            params: TableValues::new(NO_PARAMS),
            effect_clips: Vec::new(),
            groups_open: 0,
            reach: 0.0,
            mark_payloads: HashMap::new(),
            pages: HashSet::new(),
            counters: FrameCounters::default(),
            reads_time: false,
            item: 0,
            batched_item: 0,
            union_marks: Vec::new(),
        }
    }
}

impl Builder<'_> {
    /// Returns the frame slot and world matrix of the client's transform node at `slot`.
    pub(crate) fn space(&mut self, slot: u32) -> Option<(u32, Matrix4)> {
        let world = self.worlds.of(slot)?;
        let (frame_slot, first) = self.slots.spaces.slot(&(self.env.client_id, slot));
        if first {
            self.spaces.set(frame_slot, world);
        }
        Some((frame_slot, world))
    }

    /// Returns the page of the client's image at `slot`, placing it when the frame first draws
    /// it.
    pub(crate) fn image_page(&mut self, slot: u32) -> Option<ImagePage> {
        let image = self.env.client.images.at(slot)?;
        let key = (self.env.client_id, slot);
        let placed = self.content.images.page(
            &mut self.content.pages,
            key,
            image,
            self.env.caps.max_texture_size,
        )?;
        self.pages.insert(placed.page);
        Some(placed)
    }
}
