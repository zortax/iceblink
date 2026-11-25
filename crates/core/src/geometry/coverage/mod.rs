//! The device pixels where the coverage of a clip link can differ between two states.

mod compare;
mod outline;
mod partial;
mod quad;

pub(crate) use self::partial::Partial;
pub(crate) use self::quad::{QuadShape, quad_change};

use iceblink::{ClipShape, DeviceRect, Matrix4};
use smallvec::SmallVec;

use self::compare::compare;
use self::outline::Outline;
use crate::geometry::bounds::{DeviceBox, INK_LIMIT, axis_scale, map, of_pixels};

/// How many boxes a change holds before they are merged into their bounding box.
const MOST_BOXES: usize = 16;

/// A region of device pixels whose clip coverage can have changed.
///
/// The region is a set of boxes. An empty change holds no pixel.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CoverageChange {
    boxes: SmallVec<[DeviceBox; 4]>,
}

impl CoverageChange {
    /// The change that holds every pixel.
    pub(crate) fn whole() -> Self {
        let mut boxes = SmallVec::new();
        boxes.push(DeviceBox::EVERYWHERE);
        Self { boxes }
    }

    /// Reports whether the change holds every pixel.
    pub(crate) fn is_whole(&self) -> bool {
        self.boxes.contains(&DeviceBox::EVERYWHERE)
    }

    /// Adds the pixels of `other` to the change.
    pub(crate) fn add(&mut self, other: &Self) {
        for &held in &other.boxes {
            self.push(held);
        }
    }

    fn push(&mut self, added: DeviceBox) {
        if added.is_empty() || self.boxes.contains(&DeviceBox::EVERYWHERE) {
            return;
        }
        if added == DeviceBox::EVERYWHERE {
            *self = Self::whole();
            return;
        }

        self.boxes.push(added);
        if self.boxes.len() > MOST_BOXES {
            let all = self.boxes.iter().fold(added, |all, held| all.union(*held));
            self.boxes.clear();
            self.boxes.push(all);
        }
    }

    /// Reports whether the change holds or borders a point of `area`.
    pub(crate) fn meets(&self, area: DeviceBox) -> bool {
        self.boxes.iter().any(|held| {
            held.x0 <= area.x1 && area.x0 <= held.x1 && held.y0 <= area.y1 && area.y0 <= held.y1
        })
    }

    /// Returns the parts of `ink` inside the change, in whole pixels.
    pub(crate) fn cut(&self, ink: DeviceRect) -> impl Iterator<Item = DeviceRect> + '_ {
        let ink = of_pixels(ink);
        self.boxes
            .iter()
            .filter_map(move |held| held.intersect(ink).pixels(INK_LIMIT))
    }
}

/// A clip link's shape with the world matrix of its space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlacedLink<'a> {
    pub(crate) shape: &'a ClipShape,
    pub(crate) world: &'a Matrix4,
}

/// Returns the device pixels where links `old` and `new` can cover differently.
///
/// The change depends on the links:
///
/// - For rectangles without radii in spaces that scale and translate, the change is exact.
/// - For rounded rectangles in such spaces, the change holds the moved edges and the corners
///   whose position, radii or exponent changed.
/// - For rectangles without radii in one space of another kind, the change holds the moved edges
///   mapped onto the device.
/// - Links whose outlines cannot be compared, such as two different masks or rounded rectangles
///   in spaces that turn, change every pixel.
pub(crate) fn link_change(old: PlacedLink<'_>, new: PlacedLink<'_>) -> CoverageChange {
    if old.shape == new.shape && old.world == new.world {
        return CoverageChange::default();
    }
    let (
        ClipShape::RoundedRect {
            rect: old_rect,
            radii: old_radii,
            shapes: old_shapes,
        },
        ClipShape::RoundedRect {
            rect: new_rect,
            radii: new_radii,
            shapes: new_shapes,
        },
    ) = (old.shape, new.shape)
    else {
        return CoverageChange::whole();
    };

    let mut change = CoverageChange::default();

    if let (Some(old_map), Some(new_map)) = (axis_scale(old.world), axis_scale(new.world)) {
        let outlines = (
            Outline::on_device(*old_rect, *old_radii, *old_shapes, old_map),
            Outline::on_device(*new_rect, *new_radii, *new_shapes, new_map),
        );
        let (Some(before), Some(after)) = outlines else {
            return CoverageChange::whole();
        };
        compare(&before, &after, &mut |found| change.push(found));
        return change;
    }

    let square = |radii: &[f32; 8]| radii.iter().all(|&radius| radius == 0.0);
    if old.world != new.world || !square(old_radii) || !square(new_radii) {
        return CoverageChange::whole();
    }

    let outlines = (Outline::in_space(*old_rect), Outline::in_space(*new_rect));
    let (Some(before), Some(after)) = outlines else {
        return CoverageChange::whole();
    };
    // Outside both rectangles grown by the ramp, coverage is zero in both.
    let reach = before.bounds().union(after.bounds());
    compare(&before, &after, &mut |found| {
        let found = found.intersect(reach);
        if !found.is_empty() {
            change.push(map(new.world, found.rect()));
        }
    });
    change
}
