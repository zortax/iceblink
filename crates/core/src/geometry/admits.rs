//! Clip chains that cut nothing inside a region, before and after a scroll.

use iceblink::{ClipShape, Matrix4};

use crate::geometry::bounds::DeviceBox;
use crate::geometry::clip::{link_box, shape_rect};
use crate::geometry::coverage::Partial;
use crate::scene::Scene;
use crate::scene::follows::Follows;

/// Reports whether the chain of clip node `slot` admits all of `region`, and keeps admitting
/// what moves with scroll node `scroll` when that scroll node moves.
///
/// Every link must follow `scroll`, or be a still rectangle without radii in an axis-aligned
/// space, and the still links' bounds must hold `region`. With `scroll` at `None`, every link
/// must be still.
pub(crate) fn admits(scene: &Scene, slot: u32, region: DeviceBox, scroll: Option<u32>) -> bool {
    let Some(record) = scene.clips.at(slot) else {
        return false;
    };
    if !holds(record.split.still, region) {
        return false;
    }

    let mut current = Some(slot);
    let mut steps = 0;
    while let Some(slot) = current {
        let Some(link) = scene.clips.at(slot) else {
            return false;
        };
        let Some(space) = scene.transforms.at(link.node.space.slot()) else {
            return false;
        };

        let passes = match space.follows {
            Follows::Scroll(slot) => scroll == Some(slot),
            Follows::Still => axis_aligned(&space.world) && square(&link.node.shape),
            Follows::Mixed => false,
        };
        if !passes {
            return false;
        }

        current = link.node.parent.map(|parent| parent.slot());
        steps += 1;
        if steps > scene.clips.span() {
            return false;
        }
    }
    true
}

/// How the space at a slot moved while a scroll node moved: `Some(true)` with the scroll node,
/// `Some(false)` not at all, and `None` in another way or when that is unknown.
pub(crate) type Moved<'a> = &'a dyn Fn(u32) -> Option<bool>;

/// A `Moved` that knows nothing of how any space moved.
///
/// Every link whose space follows several scroll offsets then fails.
pub(crate) const UNKNOWN: Moved<'static> = &|_| None;

/// Returns where the chain of clip node `slot` covers `region` in part, when every link either
/// follows scroll node `scroll` or holds all of `region` in its box.
///
/// Each link counts as one that moves with `scroll` or as a still one:
///
/// - A link that follows `scroll` moves with it.
/// - A link that follows another scroll node is still if `others` is true, and fails otherwise.
/// - A link that follows no scroll node is still.
/// - A link whose space follows several offsets moves or is still as `moved` reports for its
///   space, and fails when `moved` does not know.
///
/// Returns `None` when a link fails or is missing, when a still link does not hold `region`, or
/// when where a still link covers in part cannot be bounded.
pub(crate) fn keeps_whole(
    scene: &Scene,
    slot: u32,
    region: DeviceBox,
    scroll: u32,
    others: bool,
    moved: Moved<'_>,
) -> Option<Partial> {
    let mut partial = Partial::NONE;
    let mut current = Some(slot);
    let mut steps = 0;
    while let Some(slot) = current {
        let link = scene.clips.at(slot)?;
        let space = scene.transforms.at(link.node.space.slot())?;
        let shape = &link.node.shape;
        let still = match space.follows {
            Follows::Scroll(follows) if follows == scroll => false,
            Follows::Scroll(_) if others => true,
            Follows::Still => true,
            Follows::Scroll(_) => return None,
            Follows::Mixed => !moved(link.node.space.slot())?,
        };

        if still {
            let bounds = link_box(&space.world, shape, shape_rect(shape));
            if !holds(bounds, region) {
                return None;
            }
            partial = partial.and(Partial::of_link(&space.world, shape)?);
        }

        current = link.node.parent.map(|parent| parent.slot());
        steps += 1;
        if steps > scene.clips.span() {
            return None;
        }
    }
    Some(partial)
}

/// Reports whether a matrix is a translation or a positive scale in the plane.
pub(crate) fn axis_aligned(matrix: &Matrix4) -> bool {
    let m = matrix.columns;
    matrix.is_2d() && m[0][1] == 0.0 && m[1][0] == 0.0 && m[0][0] > 0.0 && m[1][1] > 0.0
}

/// Reports whether a clip shape is a rectangle without radii.
pub(crate) fn square(shape: &ClipShape) -> bool {
    match shape {
        ClipShape::RoundedRect { radii, .. } => radii.iter().all(|&radius| radius == 0.0),
        ClipShape::Mask { .. } => false,
    }
}

/// Reports whether `outer` holds `inner`.
pub(crate) fn holds(outer: DeviceBox, inner: DeviceBox) -> bool {
    outer.x0 <= inner.x0 && outer.y0 <= inner.y0 && outer.x1 >= inner.x1 && outer.y1 >= inner.y1
}
