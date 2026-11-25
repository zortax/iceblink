//! Filled and stroked paths.

use std::sync::Arc;

use crate::bits::enumeration;
use crate::item::{ClipRef, SpaceRef};
use crate::paint::PaintRef;

/// A path filled, stroked or both.
///
/// The fill is drawn first and the stroke over it. Both are in path space, so the stroke width and
/// dashes scale with `placement` and the item's transform.
///
/// The ink is the control-point bounding box of `path` outset by the stroke's reach, mapped through
/// `placement` and cut to the bounding box of `clips`.
///
/// Compositors and renderers may key caches on the address of `path`, so geometry drawn again
/// keeps the same `Arc`.
#[derive(Clone, Debug)]
pub struct PathItem {
    /// The outline.
    pub path: Arc<kurbo::BezPath>,
    /// The map from path space into the item's local space.
    pub placement: kurbo::Affine,
    /// The map from paint space into path space.
    pub brush: kurbo::Affine,
    /// The fill, if the path is filled.
    pub fill: Option<PathFill>,
    /// The stroke, if the path is stroked.
    pub stroke: Option<PathStroke>,
    /// Outlines in path space. The item draws only inside all of them.
    pub clips: Arc<[PathClip]>,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
}

/// The fill of a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathFill {
    /// The paint inside the path.
    pub paint: PaintRef,
    /// The rule that decides what is inside.
    pub rule: FillRule,
}

/// The stroke of a path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathStroke {
    /// The paint of the stroke.
    pub paint: PaintRef,
    /// The width, joins, caps and dashes, in path space.
    pub style: kurbo::Stroke,
}

/// An outline a path item is cut to.
#[derive(Clone, Debug)]
pub struct PathClip {
    /// The outline, in path space.
    pub path: Arc<kurbo::BezPath>,
    /// The rule that decides what is inside.
    pub rule: FillRule,
}

enumeration! {
    /// The rule that decides which points a path encloses.
    pub struct FillRule(u8) {
        /// Points with a nonzero winding number.
        NON_ZERO = 0,
        /// Points with an odd winding number.
        EVEN_ODD = 1,
    }
}
