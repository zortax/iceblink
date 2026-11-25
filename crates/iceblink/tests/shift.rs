//! The geometry of shifts: destination, source, exposed bands and carried damage.

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, Shift};

fn rect(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

fn bands(shift: &Shift) -> Vec<DeviceRect> {
    let mut out = Vec::new();
    shift.for_each_exposed(|band| out.push(band));
    out
}

fn region() -> DeviceRect {
    rect(10, 20, 100, 200)
}

#[test]
fn a_downward_scroll_reads_below_and_exposes_the_bottom_band() {
    let shift = Shift {
        region: region(),
        by: [0, -30],
    };

    assert_eq!(shift.destination(), Some(rect(10, 20, 100, 170)));
    assert_eq!(shift.source(), Some(rect(10, 50, 100, 170)));
    assert_eq!(bands(&shift), [rect(10, 190, 100, 30)]);
}

#[test]
fn an_upward_scroll_exposes_the_top_band() {
    let shift = Shift {
        region: region(),
        by: [0, 30],
    };

    assert_eq!(bands(&shift), [rect(10, 20, 100, 30)]);
}

#[test]
fn a_diagonal_scroll_exposes_two_disjoint_bands_that_complete_the_region() {
    let shift = Shift {
        region: region(),
        by: [-10, -30],
    };
    let exposed = bands(&shift);
    let kept = shift.destination().unwrap();

    assert_eq!(exposed.len(), 2);
    assert_eq!(exposed[0].intersection(exposed[1]), None);
    let area = |r: DeviceRect| r.size.width * r.size.height;
    let covered: i32 = exposed.iter().map(|band| area(*band)).sum();
    assert_eq!(covered + area(kept), area(region()));
}

#[test]
fn a_move_past_the_whole_region_exposes_all_of_it() {
    let shift = Shift {
        region: region(),
        by: [0, -500],
    };

    assert_eq!(shift.destination(), None);
    assert_eq!(bands(&shift), [region()]);
}

#[test]
fn no_movement_exposes_nothing() {
    let shift = Shift {
        region: region(),
        by: [0, 0],
    };

    assert_eq!(shift.destination(), Some(region()));
    assert!(bands(&shift).is_empty());
}

#[test]
fn a_carried_rectangle_moves_with_the_region_and_stays_inside_it() {
    let shift = Shift {
        region: region(),
        by: [0, -30],
    };

    assert_eq!(shift.carry(rect(20, 0, 10, 60)), Some(rect(20, 20, 10, 10)));
    assert_eq!(shift.carry(rect(200, 40, 10, 10)), None);
    assert_eq!(shift.carry(rect(20, 30, 10, 10)), None);
}

#[test]
fn carried_damage_keeps_what_lies_outside_and_moves_what_lies_inside() {
    let shift = Shift {
        region: rect(0, 100, 100, 100),
        by: [0, -30],
    };
    let mut owed = DamageSet::<6>::new();
    owed.absorb(rect(10, 80, 20, 40));
    owed.absorb(rect(50, 150, 20, 20));
    let mut damage = DamageSet::<12>::new();

    shift.carry_set(&owed, &mut damage);

    assert!(damage.contains(rect(10, 80, 20, 20)));
    assert!(!damage.intersects(rect(10, 100, 20, 20)));
    assert!(damage.contains(rect(50, 120, 20, 20)));
    assert!(!damage.intersects(rect(50, 140, 20, 30)));
}

#[test]
fn full_owed_damage_makes_the_carried_damage_full() {
    let shift = Shift {
        region: region(),
        by: [0, -30],
    };
    let mut damage = DamageSet::<6>::new();

    shift.carry_set(&DamageSet::<6>::full(), &mut damage);

    assert!(damage.is_full());
}
