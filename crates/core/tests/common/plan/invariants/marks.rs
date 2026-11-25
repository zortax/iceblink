//! The mark plan's bins.

use std::collections::HashMap;

use iceblink::damage::DamageSet;
use iceblink::geom::Point;
use iceblink::{DeviceRect, Frame, MAX_MARK_PAGES};
use iceblink_core::plan::LaneKind;

use crate::common::plan::scene::{Pushed, Scene};

use super::output;

/// Checks the mark plan: one bin per drawn union mark, holding its damaged ink, disjoint bins,
/// and overflow on the spare page.
pub(super) fn check_mark_plan(scene: &Scene, damage: &DamageSet, frame: &Frame) {
    let plan = &frame.mark_plan;
    assert_eq!(plan.full_damage, damage.is_full());
    assert!(plan.bins.windows(2).all(|pair| pair[0].item < pair[1].item));
    assert!(
        plan.overflow
            .windows(2)
            .all(|pair| pair[0].item < pair[1].item)
    );

    let sources: Vec<_> = frame.lanes.marks.instances().collect();
    let mut unions = HashMap::new();
    for logged in &scene.log {
        if let Pushed::Instance {
            lane: LaneKind::Marks,
            index,
            union,
            ..
        } = logged.pushed
        {
            unions.insert(index, (union, logged.ink));
        }
    }

    for (position, source) in sources.iter().enumerate() {
        let (union, ink) = unions[&source.index];
        let found = plan
            .bin(position as u32)
            .or(plan.overflow_bin(position as u32));
        assert_eq!(union, found.is_some(), "a drawn union mark has a bin");
        assert!(
            plan.bin(position as u32).is_none() || plan.overflow_bin(position as u32).is_none(),
            "a mark has one bin"
        );
        let Some(bin) = found else { continue };
        let on_output = ink
            .intersection(output(scene))
            .expect("a drawn mark is on the output");
        assert!(
            on_output.contains_rect(bin.region),
            "a bin holds only the mark's ink"
        );
        if damage.is_full() {
            assert_eq!(bin.region, on_output);
        }
        for piece in damage.rects() {
            if let Some(needed) = on_output.intersection(*piece) {
                assert!(
                    bin.region.contains_rect(needed),
                    "a bin holds the damaged ink"
                );
            }
        }
        assert!(bin.page < plan.pages);
        assert!(bin.at[0] + bin.region.size.width as u32 <= plan.extent[0]);
        assert!(bin.at[1] + bin.region.size.height as u32 <= plan.extent[1]);
    }

    assert!(plan.extent[0] <= scene.size[0] && plan.extent[1] <= scene.size[1]);
    for bin in &plan.overflow {
        assert_eq!(
            (bin.page, bin.at),
            (MAX_MARK_PAGES, [0, 0]),
            "overflow is on the spare page"
        );
    }
    for (at, left) in plan.bins.iter().enumerate() {
        assert!(left.page < MAX_MARK_PAGES);
        for right in &plan.bins[at + 1..] {
            if left.page == right.page {
                let place = |bin: &iceblink::MarkBin| {
                    DeviceRect::new(
                        Point::new(bin.at[0] as i32, bin.at[1] as i32),
                        bin.region.size,
                    )
                };
                assert!(!place(left).intersects(place(right)), "bins do not overlap");
            }
        }
    }
}
