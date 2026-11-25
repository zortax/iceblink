//! Paths in a frame.

use std::sync::Arc;

use crate::geometry::DeviceRect;
use crate::item::{PathClip, PathFill, PathStroke};

/// A path, placed on the device.
///
/// A path under a projective world matrix is passed with its points mapped through the matrix
/// and an identity `transform`. The paints are frame paint slots.
#[derive(Clone, Debug)]
pub struct FramePath {
    /// The outline.
    pub path: Arc<kurbo::BezPath>,
    /// The map from path space to device space.
    pub transform: kurbo::Affine,
    /// The map from paint space into path space.
    pub brush: kurbo::Affine,
    /// The fill, if the path is filled.
    pub fill: Option<PathFill>,
    /// The stroke, if the path is stroked.
    pub stroke: Option<PathStroke>,
    /// Outlines in path space. The path draws only inside all of them.
    pub clips: Arc<[PathClip]>,
    /// The device pixels the path can change.
    pub ink: DeviceRect,
}
