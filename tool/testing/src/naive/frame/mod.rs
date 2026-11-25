//! Lowering the scene an output draws into the lanes, arrays, batches, plans and tables of a frame.

mod builder;
mod chain;
mod checkpoint;
mod emit;
mod item;
mod lower;
mod mark_plan;
mod paint;
mod shaders;
mod slots;
mod walk;

use std::collections::HashSet;

use iceblink::{
    ClientId, ClipEntry, Frame, FrameCounters, FramePaint, Matrix4, OrderId, ParamsBlock,
    RenderCapabilities, SubpixelOrder,
};
use iceblink::{FontRegistry, GlyphRaster, OutputConfig};

use crate::naive::content::Content;
use crate::naive::geometry::{Bounds, output_rect};
use crate::naive::scene::Scene;

pub(crate) use self::shaders::ShaderSlots;
pub(crate) use self::slots::{OutputSlots, TableValues};

use self::builder::{Builder, Env};

/// The scene and output state a frame is lowered from.
pub(crate) struct Input<'a> {
    pub(crate) scene: &'a Scene,
    /// The order node the output draws, with the client that owns it.
    pub(crate) root: Option<(ClientId, OrderId)>,
    pub(crate) config: OutputConfig,
    pub(crate) caps: RenderCapabilities,
    pub(crate) subpixel: Option<SubpixelOrder>,
    pub(crate) raster: Option<&'a dyn GlyphRaster>,
    pub(crate) fonts: &'a dyn FontRegistry,
    pub(crate) content: &'a mut Content,
    pub(crate) slots: &'a mut OutputSlots,
    pub(crate) shaders: &'a mut ShaderSlots,
}

/// The results of lowering a frame, besides the frame itself.
pub(crate) struct Lowered {
    pub(crate) counters: FrameCounters,
    /// Whether the frame draws with a shader that reads the clock.
    pub(crate) reads_time: bool,
    /// The pages the frame draws from.
    pub(crate) pages: HashSet<u32>,
}

/// Lowers everything the output draws into `frame`, whose lists are empty.
///
/// Every table is replaced whole, and every instance is transient.
pub(crate) fn lower(input: Input<'_>, frame: &mut Frame) -> Lowered {
    let output = output_rect(input.config.size);
    let Some((client_id, root, client)) = input
        .root
        .and_then(|(id, root)| Some((id, root, input.scene.clients.get(&id)?)))
    else {
        replace_empty(frame);
        input.slots.finish_frame();
        return Lowered {
            counters: FrameCounters::default(),
            reads_time: false,
            pages: HashSet::new(),
        };
    };

    let env = Env {
        client_id,
        client,
        raster: input.raster,
        fonts: input.fonts,
        caps: input.caps,
        subpixel: input.subpixel,
        config: input.config,
        output: Bounds::of_device(output),
    };
    let mut builder = Builder::new(frame, env, input.slots, input.shaders, input.content);
    walk::node(&mut builder, root);

    let Builder {
        frame,
        slots,
        spaces,
        clips,
        paints,
        params,
        mut counters,
        reads_time,
        pages,
        union_marks,
        ..
    } = builder;

    mark_plan::plan(&mut frame.mark_plan, &union_marks, output);
    counters.mark_bins = (frame.mark_plan.bins.len() + frame.mark_plan.overflow.len()) as u32;

    spaces.replace(&mut frame.tables.spaces);
    clips.replace(&mut frame.tables.clips);
    paints.replace(&mut frame.tables.paints);
    params.replace(&mut frame.tables.params);
    slots.finish_frame();

    Lowered {
        counters,
        reads_time,
        pages,
    }
}

/// Replaces every table with an empty one, for a frame that draws nothing.
fn replace_empty(frame: &mut Frame) {
    TableValues::new(Matrix4::IDENTITY).replace(&mut frame.tables.spaces);
    TableValues::new(ClipEntry::default()).replace(&mut frame.tables.clips);
    TableValues::new(FramePaint::None).replace(&mut frame.tables.paints);
    TableValues::new(NO_PARAMS).replace(&mut frame.tables.params);
}

/// A parameter block of zeros, the value of parameter slots without a block.
pub(crate) const NO_PARAMS: ParamsBlock = ParamsBlock {
    pointer: [0.0; 2],
    hovered: 0.0,
    _reserved: 0.0,
    user: [0; 64],
};
