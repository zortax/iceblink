//! Paths lowered into frame paths: their device transform and their ink.

mod ink;
pub(crate) mod project;

use std::sync::Arc;

use iceblink::geom::{Affine2, Matrix4};
use iceblink::{FramePath, PathClip, PathFill, PathStroke};
use kurbo::{Affine, BezPath};

pub(crate) use self::ink::{device_ink, stroke_reach};

/// A path in its own space and how it is placed, before its world matrix is applied.
#[derive(Clone, Debug)]
pub(crate) struct PlacedPath<'a> {
    pub(crate) path: &'a Arc<BezPath>,
    /// Maps path space into local space, chunk offset included.
    pub(crate) placement: Affine,
    pub(crate) brush: Affine,
    pub(crate) fill: Option<PathFill>,
    pub(crate) stroke: Option<PathStroke>,
    pub(crate) clips: &'a Arc<[PathClip]>,
}

/// Returns the frame path of `placed` under the world matrix `world`.
///
/// The frame path depends on the world matrix:
///
/// - Under a two-dimensional matrix, the path keeps its allocation and its transform maps path
///   space to device space.
/// - Under a projective matrix, the points of the path and of its clips are mapped onto the
///   device, and the transform is the identity. A path with a point whose `W` is zero or below is
///   left empty.
pub(crate) fn frame_path(placed: PlacedPath<'_>, world: &Matrix4) -> FramePath {
    let reach = placed.stroke.as_ref().map_or(0.0, stroke_reach);
    match world.to_affine2() {
        Some(affine) => flat_path(placed, affine, reach),
        None => projected_path(placed, world, reach),
    }
}

/// Returns the frame path of `placed` under the two-dimensional world matrix `affine`, for a
/// stroke that extends `reach` outside the path.
fn flat_path(placed: PlacedPath<'_>, affine: Affine2, reach: f64) -> FramePath {
    let transform = affine_of(affine) * placed.placement;
    let mut bounds = placed.path.control_box().inflate(reach, reach);
    if let Some(clipped) = clip_box(placed.clips) {
        bounds = bounds.intersect(clipped);
    }

    FramePath {
        path: Arc::clone(placed.path),
        transform,
        brush: placed.brush,
        fill: placed.fill,
        stroke: placed.stroke,
        clips: Arc::clone(placed.clips),
        ink: device_ink(bounds, transform),
    }
}

/// Returns the frame path of `placed` mapped onto the device through the projective world matrix
/// `world`, for a stroke that extends `reach` outside the path.
fn projected_path(placed: PlacedPath<'_>, world: &Matrix4, reach: f64) -> FramePath {
    // A path with a point whose W is zero or below is not drawn.
    let path = if project::in_front(placed.path, placed.placement, world) {
        project::project(placed.path, placed.placement, world)
    } else {
        BezPath::new()
    };
    let clips: Arc<[PathClip]> = placed
        .clips
        .iter()
        .map(|clip| PathClip {
            path: Arc::new(project::project(&clip.path, placed.placement, world)),
            rule: clip.rule,
        })
        .collect();

    let mut bounds = path.control_box().inflate(reach, reach);
    if let Some(clipped) = clip_box(&clips) {
        bounds = bounds.intersect(clipped);
    }

    FramePath {
        path: Arc::new(path),
        transform: Affine::IDENTITY,
        brush: placed.brush,
        fill: placed.fill,
        stroke: placed.stroke,
        clips,
        ink: device_ink(bounds, Affine::IDENTITY),
    }
}

/// Returns the two-dimensional transform as a kurbo affine.
pub(crate) fn affine_of(affine: Affine2) -> Affine {
    Affine::new([
        f64::from(affine.a),
        f64::from(affine.b),
        f64::from(affine.c),
        f64::from(affine.d),
        f64::from(affine.tx),
        f64::from(affine.ty),
    ])
}

/// Returns the intersection of the control boxes of `clips`, or `None` without clips.
fn clip_box(clips: &[PathClip]) -> Option<kurbo::Rect> {
    clips
        .iter()
        .map(|clip| clip.path.control_box())
        .reduce(|first, second| first.intersect(second))
}
