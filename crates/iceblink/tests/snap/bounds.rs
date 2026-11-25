//! Rectangle snapping and covering.

use iceblink::geom::snap::{cover_device_bounds, snap_device_bounds};
use iceblink::geom::{
    Device, DevicePx, Logical, LogicalPx, Point, Rect, Scale, Size, cover_bounds, snap_bounds,
};
use proptest::prelude::*;

/// A well-formed logical rectangle and a plausible scale factor.
fn case() -> impl Strategy<Value = (Rect<LogicalPx, Logical>, Scale<Logical, Device>)> {
    (
        -4000.0_f32..4000.0,
        -4000.0_f32..4000.0,
        0.0_f32..4000.0,
        0.0_f32..4000.0,
        0.25_f32..4.0,
    )
        .prop_map(|(x, y, width, height, factor)| {
            (
                Rect::new(
                    Point::new(LogicalPx(x), LogicalPx(y)),
                    Size::new(LogicalPx(width), LogicalPx(height)),
                ),
                Scale::new(factor),
            )
        })
}

#[test]
fn a_sub_pixel_rectangle_may_collapse_but_never_inverts() {
    let scale: Scale<Logical, Device> = Scale::new(1.0);
    let sliver: Rect<LogicalPx, Logical> = Rect::new(
        Point::new(LogicalPx(10.0), LogicalPx(10.0)),
        Size::new(LogicalPx(0.2), LogicalPx(0.2)),
    );
    let snapped = snap_bounds(sliver, scale);
    assert_eq!(snapped.size, Size::new(DevicePx(0.0), DevicePx(0.0)));
    assert!(snapped.right() >= snapped.left());
    assert!(snapped.bottom() >= snapped.top());
}

#[test]
fn covering_keeps_a_sub_pixel_rectangle_visible() {
    let scale: Scale<Logical, Device> = Scale::new(1.0);
    let sliver: Rect<LogicalPx, Logical> = Rect::new(
        Point::new(LogicalPx(10.1), LogicalPx(10.1)),
        Size::new(LogicalPx(0.2), LogicalPx(0.2)),
    );
    assert_eq!(
        cover_bounds(sliver, scale).size,
        Size::new(DevicePx(1.0), DevicePx(1.0))
    );
}

proptest! {
    /// Snapping an already-snapped rectangle changes nothing.
    #[test]
    fn snapping_is_idempotent((rect, scale) in case()) {
        let once = snap_bounds(rect, scale);
        prop_assert_eq!(snap_device_bounds(once), once);
        prop_assert_eq!(snap_device_bounds(snap_device_bounds(once)), once);
    }

    /// Covering an already-covered rectangle changes nothing.
    #[test]
    fn covering_is_idempotent((rect, scale) in case()) {
        let once = cover_bounds(rect, scale);
        prop_assert_eq!(cover_device_bounds(once), once);
    }

    /// A snapped rectangle sits exactly on the device pixel grid.
    #[test]
    fn snapped_edges_land_on_the_grid((rect, scale) in case()) {
        let snapped = snap_bounds(rect, scale);
        prop_assert!(snapped.left().is_grid_aligned());
        prop_assert!(snapped.top().is_grid_aligned());
        prop_assert!(snapped.right().is_grid_aligned());
        prop_assert!(snapped.bottom().is_grid_aligned());
    }

    /// The covering rectangle contains the rectangle it was computed from.
    #[test]
    fn covering_contains_the_original((rect, scale) in case()) {
        let scaled = rect * scale;
        let covered = cover_bounds(rect, scale);
        prop_assert!(covered.left() <= scaled.left());
        prop_assert!(covered.top() <= scaled.top());
        prop_assert!(covered.right() >= scaled.right());
        prop_assert!(covered.bottom() >= scaled.bottom());
        prop_assert!(covered.contains_rect(scaled), "{:?} does not contain {:?}", covered, scaled);
    }

    /// Snapping moves no edge by more than half a device pixel.
    #[test]
    fn snapping_moves_nothing_far((rect, scale) in case()) {
        let scaled = rect * scale;
        let snapped = snap_bounds(rect, scale);
        prop_assert!((snapped.left() - scaled.left()).abs() <= DevicePx(0.5));
        prop_assert!((snapped.top() - scaled.top()).abs() <= DevicePx(0.5));
    }
}
