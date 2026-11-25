//! Extents.

use iceblink::geom::{Logical, LogicalPx, Size};

#[test]
fn emptiness_covers_zero_and_negative_extents() {
    let empty: Size<LogicalPx, Logical> = Size::new(LogicalPx(0.0), LogicalPx(10.0));
    assert!(empty.is_empty());
    assert!(Size::<LogicalPx, Logical>::new(LogicalPx(-1.0), LogicalPx(10.0)).is_empty());
    assert!(!Size::<LogicalPx, Logical>::new(LogicalPx(1.0), LogicalPx(10.0)).is_empty());
}

#[test]
fn negative_extents_clamp_to_zero() {
    let size: Size<LogicalPx, Logical> = Size::new(LogicalPx(-4.0), LogicalPx(6.0));
    assert_eq!(
        size.non_negative(),
        Size::new(LogicalPx(0.0), LogicalPx(6.0))
    );
}

#[test]
fn extents_add_componentwise() {
    let a: Size<LogicalPx, Logical> = Size::new(LogicalPx(1.0), LogicalPx(2.0));
    assert_eq!(a + a, Size::new(LogicalPx(2.0), LogicalPx(4.0)));
}
