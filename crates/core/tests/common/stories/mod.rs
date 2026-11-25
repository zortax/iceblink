//! The stories the vertical-slice tests play: boxes, text, images, paths, marks, groups,
//! scrolling and clip writes.

mod boxes;
mod clips;
mod groups;
mod images;
mod marks;
mod paths;
mod scroll;
mod text;

use std::sync::Arc;

use iceblink::color::{Color as StopColor, ColorSpace};
use iceblink::{
    ChunkKey, Gradient, GradientShape, GradientStop, Interpolation, Paint, Transaction,
};

use crate::common::story::{Story, World};

pub(crate) use self::boxes::BOXES;
pub(crate) use self::clips::CLIPS;
pub(crate) use self::groups::GROUPS;
pub(crate) use self::images::IMAGES;
pub(crate) use self::marks::MARKS;
pub(crate) use self::paths::PATHS;
pub(crate) use self::scroll::SCROLL;
pub(crate) use self::text::TEXT;

/// Every story.
pub(crate) const ALL: [Story; 8] = [BOXES, TEXT, IMAGES, PATHS, MARKS, GROUPS, SCROLL, CLIPS];

fn key(owner: u64) -> ChunkKey {
    ChunkKey::new(owner, 0)
}

/// Records a white background over the whole output and appends it to the root.
fn background(tx: &mut Transaction, world: &mut World) {
    let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
    world.boxes(tx, key(1), white, &[[0.0, 0.0, 128.0, 128.0]], true);
}

/// Returns a red to blue ramp over 60 pixels, interpolated in Oklab.
pub(crate) fn ramp() -> Paint {
    Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [60.0, 0.0],
        },
        stops: Arc::from(vec![
            GradientStop::new(0.0, StopColor::new(ColorSpace::Srgb, [1.0, 0.0, 0.0], 1.0)),
            GradientStop::new(1.0, StopColor::new(ColorSpace::Srgb, [0.0, 0.0, 1.0], 1.0)),
        ]),
        interpolation: Interpolation::new(ColorSpace::Oklab),
        repeat: false,
    })
}
