//! The frame tables every output shares: world matrices, clip chains, paints and parameters.

pub(crate) mod chains;
pub(crate) mod clip;
pub(crate) mod combined;
pub(crate) mod inline;
pub(crate) mod journal;
pub(crate) mod paint;
pub(crate) mod table;

use std::sync::Arc;

use iceblink::{ClipEntry, FRAME_NO_CLIP, FramePaint, Matrix4, ParamsBlock, ShaderDesc, Tables};
use rustc_hash::FxHashMap;

use crate::output::renderer::RendererState;

use self::chains::ChainIntern;
use self::combined::CombinedChains;
use self::inline::InlinePaints;
use self::table::FrameTable;

/// The frame tables and the slots of the frame's shaders.
#[derive(Debug, Default)]
pub(crate) struct FrameTables {
    pub(crate) spaces: FrameTable<Matrix4>,
    pub(crate) clips: FrameTable<ClipEntry>,
    pub(crate) paints: FrameTable<FramePaint>,
    pub(crate) params: FrameTable<ParamsBlock>,
    /// Clip chains with an outer chain added outside them.
    pub(crate) combined: CombinedChains,
    /// The next frame shader slot.
    pub(crate) next_shader: u32,
    /// The live shaders, by frame shader slot.
    pub(crate) shaders: FxHashMap<u32, Arc<ShaderDesc>>,
    /// The chains vector planning made from the links of other chains.
    pub(crate) chains: ChainIntern,
    /// The paint slots of inline colours.
    pub(crate) inline: InlinePaints,
    /// The slots a table delta lists, kept between frames for their capacity.
    slots: Vec<u32>,
}

impl FrameTables {
    /// Returns the frame clip slot of the chain `inner` with the chain `outer` outside it.
    ///
    /// Either may be `FRAME_NO_CLIP`, which adds nothing.
    pub(crate) fn combine(&mut self, outer: u32, inner: u32) -> u32 {
        if outer == FRAME_NO_CLIP {
            return inner;
        }
        if inner == FRAME_NO_CLIP {
            return outer;
        }

        self.combined.combine(&mut self.clips, outer, inner)
    }

    /// Adds a recording that uses each of the inline colours `colors`.
    pub(crate) fn hold_inline(&mut self, colors: &[[u8; 4]]) {
        for &rgba in colors {
            self.inline.hold(&mut self.paints, rgba);
        }
    }

    /// Removes a recording that used each of the inline colours `colors`, and releases at build
    /// serial `serial` the slots no recording uses any more.
    pub(crate) fn release_inline(&mut self, colors: &[[u8; 4]], serial: u64) {
        for &rgba in colors {
            self.inline.release(&mut self.paints, rgba, serial);
        }
    }

    /// Rebuilds every combined chain whose parts may have changed since the last rebuild.
    pub(crate) fn refresh_combined(&mut self) {
        self.combined.refresh(&mut self.clips);
    }

    /// Fills the table deltas of a frame against what `renderer` holds, and records that the
    /// renderer now holds the current tables.
    pub(crate) fn deltas(&mut self, renderer: &mut RendererState, tables: &mut Tables) {
        let replace = renderer.fresh;
        let [spaces, clips, paints, params] = &mut renderer.tables;
        let slots = &mut self.slots;

        self.spaces
            .delta(*spaces, replace, &mut tables.spaces, slots);
        self.clips.delta(*clips, replace, &mut tables.clips, slots);
        self.paints
            .delta(*paints, replace, &mut tables.paints, slots);
        self.params
            .delta(*params, replace, &mut tables.params, slots);

        renderer.tables = [
            self.spaces.version(),
            self.clips.version(),
            self.paints.version(),
            self.params.version(),
        ];
    }

    /// Makes reusable every slot released before build serial `settled`.
    pub(crate) fn settle(&mut self, settled: u64) {
        self.spaces.settle(settled);
        self.clips.settle(settled);
        self.paints.settle(settled);
        self.params.settle(settled);
    }

    /// Returns the bytes the tables occupy.
    pub(crate) fn bytes(&self) -> u64 {
        self.spaces.bytes() + self.clips.bytes() + self.paints.bytes() + self.params.bytes()
    }
}
