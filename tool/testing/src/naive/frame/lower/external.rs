//! External quads and backdrops, which the frame carries in arrays of their own.

use iceblink::{
    Backdrop, BatchKind, ClipRef, ExternalQuad, FrameBackdrop, FrameExternal, FrameFilter, SpaceRef,
};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::item;
use crate::naive::geometry::Bounds;
use crate::naive::scene::Chunk;

pub(crate) fn external(builder: &mut Builder<'_>, chunk: &Chunk, quad: &ExternalQuad) -> Bounds {
    let Some(placed) = item::place(builder, chunk, quad.space, quad.clip) else {
        return Bounds::EMPTY;
    };

    let ink = placed.ink(Bounds::of(quad.bounds), builder.env.output);
    let Some((texture, _)) = builder.env.client.textures.at(quad.texture) else {
        return ink;
    };
    if !item::shows(builder, ink) {
        return ink;
    }

    let mut lowered = *quad;
    lowered.bounds[0] += placed.offset[0];
    lowered.bounds[1] += placed.offset[1];
    lowered.clip = ClipRef(placed.clip);
    lowered.space = SpaceRef(placed.space);

    let index = builder.frame.externals.len() as u32;
    builder.frame.externals.push(FrameExternal {
        quad: lowered,
        client: builder.env.client_id,
        texture: *texture,
    });
    builder.counters.items_drawn += 1;
    builder.batch(BatchKind::External(index));
    ink
}

/// Lowers a backdrop with the device box of its bounds, the extent its filter chain reads around
/// that box, and the chain in frame slots, and returns its device ink.
pub(crate) fn backdrop(builder: &mut Builder<'_>, chunk: &Chunk, backdrop: &Backdrop) -> Bounds {
    let Some(placed) = item::place(builder, chunk, backdrop.space, backdrop.clip) else {
        return Bounds::EMPTY;
    };

    let device = Bounds::of(backdrop.bounds)
        .translated(placed.offset)
        .mapped(&placed.world)
        .unwrap_or(builder.env.output);
    let ink = placed.cut(device);
    let Some(chain) = chunk.recorded.filters.get(backdrop.filters as usize) else {
        return ink;
    };
    if !item::shows(builder, ink) {
        return ink;
    }

    let filters = builder.filters(chain);
    let mut lowered = *backdrop;
    lowered.bounds[0] += placed.offset[0];
    lowered.bounds[1] += placed.offset[1];
    lowered.clip = ClipRef(placed.clip);
    lowered.space = SpaceRef(placed.space);

    let bounds = device.to_rect();
    let index = builder.frame.backdrops.len() as u32;
    builder.frame.backdrops.push(FrameBackdrop {
        backdrop: lowered,
        bounds,
        source: read_extent(bounds, &filters),
        filters,
    });
    builder.counters.items_drawn += 1;
    builder.batch(BatchKind::Backdrop(index));
    ink
}

/// Returns every device pixel a composite of `bounds` under `filters` reads.
pub(crate) fn read_extent(bounds: [f32; 4], filters: &[FrameFilter]) -> [f32; 4] {
    let [mut left, mut top, mut right, mut bottom] = [0.0f32; 4];
    for filter in filters {
        let [l, t, r, b] = filter.support();
        left += l;
        top += t;
        right += r;
        bottom += b;
    }

    let [x, y, width, height] = bounds;
    [
        x - left,
        y - top,
        width + left + right,
        height + top + bottom,
    ]
}
