//! Per-side lengths.

use iceblink::geom::{Edges, Logical, LogicalPx, Size};

#[test]
fn the_shorthand_order_is_top_right_bottom_left() {
    let edges = Edges::new(
        LogicalPx(1.0),
        LogicalPx(2.0),
        LogicalPx(3.0),
        LogicalPx(4.0),
    );
    assert_eq!(
        edges.into_array(),
        [
            LogicalPx(1.0),
            LogicalPx(2.0),
            LogicalPx(3.0),
            LogicalPx(4.0)
        ]
    );
    assert_eq!(edges.horizontal(), LogicalPx(6.0));
    assert_eq!(edges.vertical(), LogicalPx(4.0));
}

#[test]
fn axes_puts_the_horizontal_length_on_left_and_right() {
    let edges = Edges::axes(LogicalPx(10.0), LogicalPx(2.0));
    assert_eq!(edges.left, LogicalPx(10.0));
    assert_eq!(edges.right, LogicalPx(10.0));
    assert_eq!(edges.top, LogicalPx(2.0));
    assert_eq!(edges.bottom, LogicalPx(2.0));
}

#[test]
fn the_total_is_a_size_in_the_requested_space() {
    let edges = Edges::uniform(LogicalPx(3.0));
    let total: Size<LogicalPx, Logical> = edges.total();
    assert_eq!(total, Size::new(LogicalPx(6.0), LogicalPx(6.0)));
}
