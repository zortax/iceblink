//! Builders shared by the plan tests.

pub(crate) mod clips;
pub(crate) mod fixture;
pub(crate) mod invariants;
pub(crate) mod scene;

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Size};

pub(crate) fn rect(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    DeviceRect::new(Point::new(x, y), Size::new(width, height))
}

/// Returns a damage set of one rectangle.
pub(crate) fn damaged(x: i32, y: i32, width: i32, height: i32) -> DamageSet {
    let mut damage = DamageSet::new();
    damage.absorb(rect(x, y, width, height));
    damage
}
