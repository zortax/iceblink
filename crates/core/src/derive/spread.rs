//! How far the filters of the groups around some content spread what it draws.

use iceblink::{DeviceRect, read_extent};

use crate::geometry::bounds::{INK_LIMIT, of_pixels};
use crate::scene::Scene;

/// How far the filters of the groups around some content spread what it draws, as device pixels
/// to the left, top, right and bottom.
///
/// A change to the content changes the output of those groups up to that far from the change.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Spread([f32; 4]);

impl Spread {
    /// Returns the spread of the group the order node at `slot` forms, or no spread when it forms
    /// none.
    pub(crate) fn of_node(scene: &Scene, slot: u32) -> Self {
        let effect = scene
            .orders
            .at(slot)
            .and_then(|record| record.node.effect)
            .and_then(|effect| scene.effects.at(effect.slot()));
        match effect {
            Some(effect) if effect.node.isolates() => {
                let [x, y, width, height] = read_extent([0.0; 4], &effect.node.filters);
                Self([-x, -y, x + width, y + height])
            }
            _ => Self::default(),
        }
    }

    /// Returns the spread of the groups the order node at `slot` and its ancestors form.
    pub(crate) fn inside(scene: &Scene, slot: u32) -> Self {
        upwards(scene, slot).fold(Self::default(), |spread, node| {
            spread.and(Self::of_node(scene, node))
        })
    }

    /// Returns the spread of the groups the ancestors of the order node at `slot` form.
    pub(crate) fn above(scene: &Scene, slot: u32) -> Self {
        scene
            .node_placements
            .first(slot)
            .map_or(Self::default(), |parent| Self::inside(scene, parent))
    }

    /// Returns the spread of content inside the groups of `self`, nested in the groups of
    /// `outer`.
    pub(crate) fn and(self, outer: Self) -> Self {
        let [a, b] = [self.0, outer.0];
        Self([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]])
    }

    /// Reports whether the spread reaches past the content.
    pub(crate) fn reaches(self) -> bool {
        self.0.iter().any(|side| *side > 0.0)
    }

    /// Returns the pixels a change inside `ink` can change.
    pub(crate) fn grow(self, ink: DeviceRect) -> DeviceRect {
        if !self.reaches() {
            return ink;
        }
        of_pixels(ink)
            .outset(self.0)
            .pixels(INK_LIMIT)
            .unwrap_or(ink)
    }
}

/// Returns the order node at `slot` and its ancestors, innermost first.
pub(crate) fn upwards(scene: &Scene, slot: u32) -> impl Iterator<Item = u32> + '_ {
    // A sequence of placements without a cycle is shorter than the slab.
    let span = scene.orders.span() + 1;
    core::iter::successors(Some(slot), |current| scene.node_placements.first(*current)).take(span)
}
