//! When a scratch texture grows and shrinks.

use iceblink_wgpu::vector::{Decay, Extent};
use web_time::{Duration, Instant};

fn later(start: Instant, millis: u64) -> Instant {
    start + Duration::from_millis(millis)
}

#[test]
fn the_first_frame_allocates_exactly_what_it_asked_for() {
    let mut decay = Decay::new();
    let want = Extent::new(640, 480, 4);

    assert_eq!(decay.wants_at(want, Instant::now()), Some(want));
    assert_eq!(decay.held(), want);
}

#[test]
fn a_frame_that_needs_more_gets_it_immediately() {
    let mut decay = Decay::new();
    let now = Instant::now();
    decay.wants_at(Extent::new(640, 480, 4), now);

    assert_eq!(
        decay.wants_at(Extent::new(640, 480, 9), now),
        Some(Extent::new(640, 480, 9))
    );
}

#[test]
fn growth_is_a_union_so_a_wider_shorter_frame_keeps_the_height() {
    let mut decay = Decay::new();
    let now = Instant::now();
    decay.wants_at(Extent::new(200, 800, 4), now);

    assert_eq!(
        decay.wants_at(Extent::new(900, 100, 4), now),
        Some(Extent::new(900, 800, 4))
    );
}

#[test]
fn the_scratch_shrinks_once_the_grace_period_is_over() {
    let mut decay = Decay::new();
    let now = Instant::now();
    decay.wants_at(Extent::new(1344, 896, 40), now);
    let small = Extent::new(640, 480, 2);

    assert_eq!(decay.wants_at(small, later(now, 1_999)), None);
    assert_eq!(decay.wants_at(small, later(now, 2_000)), Some(small));
    assert_eq!(decay.held(), small);
}

#[test]
fn the_scratch_does_not_shrink_inside_a_fling() {
    let mut decay = Decay::new();
    let now = Instant::now();
    let large = Extent::new(1344, 896, 40);
    let small = Extent::new(640, 480, 2);
    decay.wants_at(large, now);

    // Most frames of a scroll are cheap, and every thirtieth one is not.
    for frame in 0..600_u64 {
        let want = if frame % 30 == 0 { large } else { small };
        assert_eq!(
            decay.wants_at(want, later(now, frame * 16)),
            None,
            "the scratch was reallocated at frame {frame} of a fling"
        );
    }
    assert_eq!(decay.held(), large);
}

#[test]
fn a_settled_scratch_shrinks_once_and_then_stays() {
    let mut decay = Decay::new();
    let now = Instant::now();
    decay.wants_at(Extent::new(1344, 896, 40), now);
    let small = Extent::new(640, 480, 2);

    assert_eq!(decay.wants_at(small, later(now, 2_000)), Some(small));
    assert_eq!(decay.wants_at(small, later(now, 10_000)), None);
}

#[test]
fn a_classed_drag_reallocates_once_per_class_crossed() {
    let mut decay = Decay::new();
    let now = Instant::now();
    let mut allocations = 0;
    for step in 0..100_u32 {
        let want = Extent::new(800 + step * 8, 600, 4).classed();
        if decay
            .wants_at(want, later(now, u64::from(step) * 16))
            .is_some()
        {
            allocations += 1;
        }
    }

    // 800 to 1592 texels crosses the classes at 1024, 1280 and 1536, after the first allocation.
    assert_eq!(allocations, 4);
}

#[test]
fn a_classed_extent_rounds_up_and_keeps_its_layers() {
    assert_eq!(Extent::new(1, 1, 7).classed(), Extent::new(256, 256, 7));
    assert_eq!(Extent::new(256, 512, 3).classed(), Extent::new(256, 512, 3));
    assert_eq!(Extent::new(257, 511, 3).classed(), Extent::new(512, 512, 3));
    assert_eq!(Extent::new(0, 0, 1).classed(), Extent::new(256, 256, 1));
}
