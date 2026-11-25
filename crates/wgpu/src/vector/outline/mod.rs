//! Clip links as outlines a rasteriser fills.

mod corner;
mod project;
mod rounded;

use iceblink::{ClipLinkEntry, FrameClipShape, Matrix4};
use kurbo::{Affine, BezPath};

pub use self::rounded::rounded_rect;

/// The outline of a clip link and the transform that maps it to device space.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipOutline {
    /// The outline.
    pub path: BezPath,
    /// The map from the outline's space to device space.
    pub transform: Affine,
}

impl ClipOutline {
    /// Returns the outline of `link`, measured through the world matrices `spaces`, or `None`
    /// for a mask link, which has no outline.
    ///
    /// A rounded rectangle keeps its link's space under a two-dimensional matrix. Under a
    /// projective matrix the outline is flattened and its points are mapped onto the device, with
    /// an identity transform. A link whose space slot lies past `spaces` is measured in device
    /// space.
    pub fn of(link: &ClipLinkEntry, spaces: &[Matrix4]) -> Option<Self> {
        let FrameClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } = &link.shape
        else {
            return None;
        };
        let path = rounded_rect(*rect, *radii, *shapes);
        let Some(matrix) = spaces.get(link.space as usize) else {
            return Some(Self {
                path,
                transform: Affine::IDENTITY,
            });
        };
        Some(match crate::vector::space::affine(matrix) {
            Some(transform) => Self { path, transform },
            None => Self {
                path: project::projected(&path, matrix),
                transform: Affine::IDENTITY,
            },
        })
    }
}
