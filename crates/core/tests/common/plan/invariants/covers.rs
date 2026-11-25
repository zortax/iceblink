//! The opaque covers of a plan.

use std::collections::HashMap;

use iceblink::{BatchKind, Frame};
use iceblink_core::plan::LaneKind;

use crate::common::plan::scene::{Pushed, Scene};

/// Checks that a frame has at most four opaque covers, each a quad drawn before every group, and
/// none if it has a backdrop.
pub(super) fn check_covers(scene: &Scene, frame: &Frame) {
    assert!(frame.opaque_covers.len() <= 4);
    let has_backdrop = scene
        .log
        .iter()
        .any(|logged| matches!(logged.pushed, Pushed::Backdrop { .. }));
    if has_backdrop {
        assert!(frame.opaque_covers.is_empty(), "a backdrop allows no cover");
    }

    let mut covers = HashMap::new();
    for logged in &scene.log {
        if let Pushed::Instance {
            lane: LaneKind::Quads,
            index,
            cover: Some(cover),
            ..
        } = logged.pushed
        {
            covers.insert(index, cover);
        }
    }

    for cover in &frame.opaque_covers {
        let batch = &frame.batches[cover.batch as usize];
        assert_eq!(batch.kind, BatchKind::Quads);
        assert!(batch.range.contains(&cover.at));
        let source = frame
            .lanes
            .quads
            .instances()
            .nth(cover.at as usize)
            .expect("a cover names an instance");
        assert_eq!(covers.get(&source.index), Some(&cover.rect));
        assert!(
            frame.batches[..cover.batch as usize]
                .iter()
                .all(|batch| !matches!(batch.kind, BatchKind::GroupBegin(_))),
            "a cover is drawn before every group"
        );
    }
}
