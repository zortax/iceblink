//! Rectangles.

use iceblink::geom::{Edges, Logical, LogicalPx, Point, Rect, Size};

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect<LogicalPx, Logical> {
    Rect::new(
        Point::new(LogicalPx(x), LogicalPx(y)),
        Size::new(LogicalPx(width), LogicalPx(height)),
    )
}

/// Returns the bits of a rectangle's four numbers, which compare NaN and signed zeros exactly.
fn bits(rect: Rect<LogicalPx, Logical>) -> [u32; 4] {
    [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ]
    .map(|value| value.0.to_bits())
}

#[test]
fn corners_in_order_give_the_rectangle_the_comparisons_give() {
    let reference = |first: Point<LogicalPx, Logical>, second: Point<LogicalPx, Logical>| {
        let min = first.min(second);
        let max = first.max(second);
        Rect::new(min, max.offset_from(min))
    };
    let values = [
        -3.25,
        -0.0,
        0.0,
        0.1,
        0.3,
        17.5,
        22_000.3,
        1e30,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    for &ax in &values {
        for &ay in &values {
            for &bx in &values {
                for &by in &values {
                    let first = Point::new(LogicalPx(ax), LogicalPx(ay));
                    let second = Point::new(LogicalPx(bx), LogicalPx(by));
                    assert_eq!(
                        bits(Rect::from_corners(first, second)),
                        bits(reference(first, second)),
                        "{first:?} {second:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_rectangle_united_with_itself_is_the_rectangle_its_corners_give() {
    let values = [
        -3.25,
        -0.0,
        0.0,
        1e-5,
        0.1,
        17.5,
        22_040.3,
        f32::INFINITY,
        f32::NAN,
    ];
    for &x in &values {
        for &y in &values {
            for &width in &values {
                for &height in &values {
                    let held = rect(x, y, width, height);
                    let own = if held.is_empty() {
                        held
                    } else {
                        Rect::from_corners(held.origin, held.far_corner())
                    };
                    assert_eq!(bits(held.union(held)), bits(own), "{held:?}");
                }
            }
        }
    }
}

#[test]
fn containment_is_half_open() {
    let unit = rect(0.0, 0.0, 1.0, 1.0);
    assert!(unit.contains(Point::new(LogicalPx(0.0), LogicalPx(0.0))));
    assert!(!unit.contains(Point::new(LogicalPx(1.0), LogicalPx(0.0))));
    assert!(!unit.contains(Point::new(LogicalPx(0.0), LogicalPx(1.0))));
}

#[test]
fn disjoint_rectangles_do_not_intersect() {
    assert_eq!(
        rect(0.0, 0.0, 1.0, 1.0).intersection(rect(2.0, 0.0, 1.0, 1.0)),
        None
    );
    assert_eq!(
        rect(0.0, 0.0, 1.0, 1.0).intersection(rect(1.0, 0.0, 1.0, 1.0)),
        None
    );
}

#[test]
fn a_union_with_an_empty_rectangle_is_the_other_one() {
    let empty = rect(50.0, 50.0, 0.0, 0.0);
    let real = rect(0.0, 0.0, 10.0, 10.0);
    assert_eq!(real.union(empty), real);
    assert_eq!(empty.union(real), real);
}

#[test]
fn insetting_past_zero_stops_at_zero() {
    let inset = rect(0.0, 0.0, 4.0, 4.0).inset(Edges::uniform(LogicalPx(10.0)));
    assert_eq!(inset.size, Size::new(LogicalPx(0.0), LogicalPx(0.0)));
    assert!(inset.is_empty());
}

#[test]
fn corners_run_clockwise_from_the_top_left() {
    let corners = rect(1.0, 2.0, 3.0, 4.0).corners();
    assert_eq!(corners[0], Point::new(LogicalPx(1.0), LogicalPx(2.0)));
    assert_eq!(corners[1], Point::new(LogicalPx(4.0), LogicalPx(2.0)));
    assert_eq!(corners[2], Point::new(LogicalPx(4.0), LogicalPx(6.0)));
    assert_eq!(corners[3], Point::new(LogicalPx(1.0), LogicalPx(6.0)));
}
