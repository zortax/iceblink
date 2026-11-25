//! Stroke width snapping.

use iceblink::geom::snap::snap_device_stroke;
use iceblink::geom::{Device, DevicePx, Logical, LogicalPx, Scale, snap_stroke};
use proptest::prelude::*;

#[test]
fn a_zero_width_stays_zero_and_a_negative_one_clamps() {
    let scale: Scale<Logical, Device> = Scale::new(3.0);
    assert_eq!(snap_stroke(LogicalPx(0.0), scale), DevicePx(0.0));
    assert_eq!(snap_stroke(LogicalPx(-4.0), scale), DevicePx(0.0));
}

proptest! {
    /// A requested stroke never rounds away, and an unrequested one never appears.
    #[test]
    fn a_requested_stroke_survives(width in 0.0_f32..64.0, factor in 0.25_f32..4.0) {
        let scale: Scale<Logical, Device> = Scale::new(factor);
        let snapped = snap_stroke(LogicalPx(width), scale);
        if width > 0.0 {
            prop_assert!(snapped >= DevicePx(1.0));
        } else {
            prop_assert_eq!(snapped, DevicePx(0.0));
        }
    }

    /// Snapping an already-snapped width changes nothing.
    #[test]
    fn stroke_snapping_is_idempotent(width in 0.0_f32..64.0, factor in 0.25_f32..4.0) {
        let scale: Scale<Logical, Device> = Scale::new(factor);
        let once = snap_stroke(LogicalPx(width), scale);
        prop_assert_eq!(snap_device_stroke(once), once);
    }

    /// A snapped width lands on the grid.
    #[test]
    fn snapped_widths_land_on_the_grid(width in 0.0_f32..64.0, factor in 0.25_f32..4.0) {
        let scale: Scale<Logical, Device> = Scale::new(factor);
        prop_assert!(snap_stroke(LogicalPx(width), scale).is_grid_aligned());
    }
}
