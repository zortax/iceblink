//! Chunks and their items: where each item is drawn, through which clips, and its device ink.

use iceblink::{ChunkKey, ClipRef, ItemKind, Matrix4, SpaceRef};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::chain::{self, Link};
use crate::naive::frame::lower;
use crate::naive::geometry::Bounds;
use crate::naive::scene::Chunk;

/// Where an item is drawn: its space, its clip chain and its chunk's offset.
pub(crate) struct Placed {
    /// The frame space slot.
    pub(crate) space: u32,
    /// The world matrix of the item's transform node.
    pub(crate) world: Matrix4,
    /// The frame clip slot of the whole chain.
    pub(crate) clip: u32,
    /// The device bounding box of the chain.
    pub(crate) clip_box: Bounds,
    pub(crate) offset: [f32; 2],
}

impl Placed {
    /// Returns the device ink of a local ink: moved by the chunk offset, mapped to the device,
    /// cut to the clip chain's box and rounded outwards.
    ///
    /// A corner that maps to a `w` of zero or below makes the ink the whole output.
    pub(crate) fn ink(&self, local: Bounds, output: Bounds) -> Bounds {
        if local.is_empty() {
            return Bounds::EMPTY;
        }

        let device = local
            .translated(self.offset)
            .mapped(&self.world)
            .unwrap_or(output);
        self.cut(device)
    }

    /// Cuts a device box to the clip chain's box and rounds it outwards.
    pub(crate) fn cut(&self, device: Bounds) -> Bounds {
        device
            .intersect(self.clip_box)
            .round_out()
            .map_or(Bounds::EMPTY, Bounds::of_device)
    }
}

/// Lowers the items of the chunk `key` in record order, and returns the chunk's device ink.
///
/// A chunk that was never recorded draws nothing.
pub(crate) fn chunk(builder: &mut Builder<'_>, key: ChunkKey) -> Bounds {
    let client = builder.env.client;
    let Some(chunk) = client.chunks.get(&key) else {
        return Bounds::EMPTY;
    };

    let drawn = builder.counters.items_drawn;
    let mut ink = Bounds::EMPTY;
    for (position, op) in chunk.recorded.ops.iter().enumerate() {
        builder.next_item();
        let index = op.index as usize;
        let recorded = &chunk.recorded;
        let item_ink = match op.kind {
            ItemKind::QUAD => lower::analytic::quad(builder, chunk, &recorded.quads[index]),
            ItemKind::SHADER_QUAD => {
                lower::analytic::shader_quad(builder, chunk, &recorded.shader_quads[index])
            }
            ItemKind::SHADOW => lower::analytic::shadow(builder, chunk, &recorded.shadows[index]),
            ItemKind::LINE => lower::analytic::line(builder, chunk, &recorded.lines[index]),
            ItemKind::GLYPH_RUN => lower::glyphs::run(builder, chunk, &recorded.glyph_runs[index]),
            ItemKind::IMAGE => lower::image::image(builder, chunk, &recorded.images[index]),
            ItemKind::MARKS => lower::marks::marks(builder, chunk, &recorded.marks[index]),
            ItemKind::PATH => {
                let id = (builder.env.client_id, key, position);
                lower::path::path(builder, chunk, &recorded.paths[index], id)
            }
            ItemKind::EXTERNAL => {
                lower::external::external(builder, chunk, &recorded.externals[index])
            }
            ItemKind::BACKDROP => {
                lower::external::backdrop(builder, chunk, &recorded.backdrops[index])
            }
            _ => Bounds::EMPTY,
        };
        ink = ink.union(item_ink);
    }

    if builder.counters.items_drawn > drawn {
        builder.counters.chunks_drawn += 1;
    }
    ink
}

/// Resolves where an item of `chunk` with `space` and `clip` is drawn.
///
/// The chain is the clips of the effects above the item that do not isolate, then the item's
/// own chain. Returns `None` when the item's transform node does not exist.
pub(crate) fn place(
    builder: &mut Builder<'_>,
    chunk: &Chunk,
    space: SpaceRef,
    clip: ClipRef,
) -> Option<Placed> {
    let transform = if space == SpaceRef::INHERIT {
        chunk.props.space.slot()
    } else {
        space.0
    };
    let (space, world) = builder.space(transform)?;

    let mut links: Vec<Link> = builder.effect_clips.clone();
    chain::item_chain(builder.env.client, chunk, clip, &mut links);
    let clip_box = chain::chain_box(&links, &mut builder.worlds);
    let clip = builder.clip(&links);

    Some(Placed {
        space,
        world,
        clip,
        clip_box,
        offset: chunk.offset,
    })
}

/// Reports whether an item with device ink `ink` shows on the output, or within what the open
/// groups' filters read past it, and so is drawn.
pub(crate) fn shows(builder: &Builder<'_>, ink: Bounds) -> bool {
    !ink.intersect(builder.reached()).is_empty()
}
