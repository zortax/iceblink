//! Whether the atlas content of a lowering still holds.

use iceblink::Matrix4;

use crate::content::{keeps_pixels, stretches};
use crate::frame::lower::View;
use crate::geometry::bounds::after_offset;
use crate::scene::chunk::{Checked, Lowered, Placement};
use crate::shared_content::Content;
use crate::work::Work;

/// Reports whether `lowered` must be made again before its chunk is drawn at chunk offset `offset`.
///
/// A lowering with atlas content is stale in these cases:
///
/// - The content cache started over since it was made, or evicted one of its tiles.
/// - A frame path of it takes masks when lowered again.
/// - A space its glyphs or masks were placed under moved them to another pixel or phase, or
///   started or stopped moving under a matrix that stretches glyph tiles while it moves.
/// - It holds frame paths, and a space's linear part changed.
///
/// A lowering found current is checked again only after the content's placement count, the
/// cache's evictions or the chunk offset changed, and then only for what those changes can
/// affect.
pub(super) fn stale(
    lowered: &mut Lowered,
    scene: View<'_>,
    offset: [f32; 2],
    content: &Content,
    work: &mut Work,
) -> bool {
    let base = lowered.base;
    let Some(stamp) = &mut lowered.content else {
        return false;
    };
    if stamp.epoch != content.epoch || stamp.rerouted {
        return true;
    }

    let now = Checked {
        placements: content.placements,
        evictions: content.cache.evictions(),
        offset,
    };
    let then = stamp.checked;
    if then == now {
        return false;
    }
    work.lowerings_checked += 1;

    if then.evictions != now.evictions && !content.cache.all_resident(&stamp.tiles) {
        return true;
    }
    let moved = (then.placements != now.placements || then.offset != offset)
        && stamp
            .worlds
            .iter()
            .any(|placement| moved_off(placement, scene, base, offset));
    if moved {
        return true;
    }
    stamp.checked = now;
    false
}

/// Reports whether the space of `placement` moved the content lowered under it at chunk offset
/// `base` off what it was lowered for, now that the chunk is drawn at `offset`.
///
/// Sprites are moved off by another pixel or phase, or by starting or stopping to move under a
/// matrix that stretches glyph tiles. Frame paths are moved off by another linear part.
fn moved_off(placement: &Placement, scene: View<'_>, base: [f32; 2], offset: [f32; 2]) -> bool {
    let Some((after, moving)) = scene
        .transforms
        .at(placement.slot)
        .map(|record| (record.world, record.moving))
    else {
        return true;
    };

    let before = placement.world;
    if placement.phased && placement.moving != moving && stretches(&after) {
        return true;
    }
    if before == after && base == offset {
        return false;
    }

    if placement.phased {
        !keeps_pixels(&after_offset(&before, base), &after_offset(&after, offset))
    } else {
        !same_linear(&before, &after)
    }
}

/// Reports whether two matrices are two-dimensional with the same linear part.
fn same_linear(before: &Matrix4, after: &Matrix4) -> bool {
    let (Some(before), Some(after)) = (before.to_affine2(), after.to_affine2()) else {
        return false;
    };
    [before.a, before.b, before.c, before.d] == [after.a, after.b, after.c, after.d]
}
