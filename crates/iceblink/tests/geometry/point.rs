//! Positions.

use iceblink::geom::{Device, Logical, LogicalPx, Point, Size};

#[test]
fn positions_add_and_subtract_within_a_space() {
    let a: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
    let b = Point::new(LogicalPx(3.0), LogicalPx(5.0));
    assert_eq!(a + b, Point::new(LogicalPx(4.0), LogicalPx(7.0)));
    assert_eq!(b - a, Point::new(LogicalPx(2.0), LogicalPx(3.0)));
    assert_eq!(b.offset_from(a), Size::new(LogicalPx(2.0), LogicalPx(3.0)));
}

#[test]
fn the_space_shows_up_in_the_debug_output() {
    let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
    let device: Point<LogicalPx, Device> = logical.cast_space();
    assert!(format!("{logical:?}").contains("logical"));
    assert!(format!("{device:?}").contains("device"));
}

#[test]
fn equality_ignores_nothing_but_the_marker() {
    let a: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
    assert_eq!(a, Point::new(LogicalPx(1.0), LogicalPx(2.0)));
    assert_ne!(a, Point::new(LogicalPx(1.0), LogicalPx(2.5)));
}
