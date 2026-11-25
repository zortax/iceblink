//! What the equal-order overlap check finds, and what it refuses to answer.

mod common;

use iceblink::DeviceRect;
use iceblink_core::plan::DrawKind;
use iceblink_core::plan::order::sweep::{DEFAULT_MAX, Ordered, overlaps};

use crate::common::plan::rect;

/// Returns a quad at draw order `order` with ink `ink`.
fn one(order: u32, ink: DeviceRect) -> Ordered {
    Ordered {
        order,
        kind: DrawKind::Quad,
        ink,
    }
}

#[test]
fn a_row_of_disjoint_boxes_at_one_order_has_no_overlap() {
    let class: Vec<Ordered> = (0..500)
        .map(|column| one(1, rect(column * 20, 0, 10, 10)))
        .collect();
    assert_eq!(overlaps(&class, DEFAULT_MAX), Ok(vec![]));
}

#[test]
fn two_boxes_sharing_an_order_and_pixels_are_found() {
    let found = overlaps(
        &[one(3, rect(0, 0, 10, 10)), one(3, rect(5, 5, 10, 10))],
        DEFAULT_MAX,
    )
    .expect("under the cap");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].order, 3);
    assert_eq!(found[0].shared, rect(5, 5, 5, 5));
}

#[test]
fn overlapping_boxes_at_different_orders_are_no_overlap() {
    let found = overlaps(
        &[one(1, rect(0, 0, 10, 10)), one(2, rect(5, 5, 10, 10))],
        DEFAULT_MAX,
    );
    assert_eq!(found, Ok(vec![]));
}

#[test]
fn boxes_sharing_a_column_but_not_a_row_do_not_meet() {
    let found = overlaps(
        &[one(1, rect(0, 0, 10, 10)), one(1, rect(2, 40, 10, 10))],
        DEFAULT_MAX,
    );
    assert_eq!(found, Ok(vec![]));
}

#[test]
fn an_order_class_past_the_cap_is_refused() {
    let class: Vec<Ordered> = (0..9)
        .map(|column| one(1, rect(column * 20, 0, 10, 10)))
        .collect();
    let refused = overlaps(&class, 4).expect_err("nine is past a cap of four");
    assert_eq!((refused.order, refused.held, refused.max), (1, 9, 4));
}

#[test]
fn an_empty_rectangle_meets_nothing() {
    let found = overlaps(
        &[one(1, rect(0, 0, 0, 0)), one(1, rect(0, 0, 10, 10))],
        DEFAULT_MAX,
    );
    assert_eq!(found, Ok(vec![]));
}
