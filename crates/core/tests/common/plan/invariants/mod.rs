//! The frame, vector plan and mark plan invariants, checked on a planned scene.

mod bindings;
mod covers;
mod keys;
mod marks;
mod order;
mod vector;

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Size};

use crate::common::plan::scene::{Planned, Scene};

use self::bindings::check_bindings;
use self::covers::check_covers;
use self::keys::draw_keys;
use self::marks::check_mark_plan;
use self::order::{check_coverage, check_groups, check_paint_order};
use self::vector::check_vector_plan;

/// Where something is drawn: its batch, then its position in the lane.
type DrawKey = (u32, u32);

/// Checks every invariant on `planned`, the plan of `scene` against `damage`.
///
/// Panics at the first invariant `planned` breaks.
pub(crate) fn check(scene: &Scene, damage: &DamageSet, planned: &Planned) {
    let frame = &planned.frame;
    let keys = draw_keys(scene, frame);

    check_coverage(scene, damage, &keys);
    check_paint_order(scene, &keys);
    check_groups(scene, frame, &keys);
    check_bindings(scene, frame);
    check_vector_plan(scene, damage, frame);
    check_mark_plan(scene, damage, frame);
    check_covers(scene, frame);

    assert_eq!(
        planned
            .planner
            .order_overlaps(usize::MAX)
            .map(|found| found.len()),
        Ok(0),
        "two objects at one order overlap"
    );
}

/// Returns the output's rectangle.
fn output(scene: &Scene) -> DeviceRect {
    DeviceRect::new(
        Point::new(0, 0),
        Size::new(scene.size[0] as i32, scene.size[1] as i32),
    )
}
