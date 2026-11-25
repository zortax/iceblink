//! The bindings of sprite and shader quad batches.

use std::collections::HashMap;

use iceblink::{BatchKind, Frame};
use iceblink_core::plan::LaneKind;

use crate::common::plan::scene::{Pushed, Scene, lane_sources};

/// Checks that every instance of a sprite or shader quad batch has the batch's binding.
pub(super) fn check_bindings(scene: &Scene, frame: &Frame) {
    let mut keys: HashMap<(LaneKind, u32), [u32; 2]> = HashMap::new();
    for logged in &scene.log {
        if let Pushed::Instance {
            lane, index, key, ..
        } = logged.pushed
        {
            keys.insert((lane, index), key);
        }
    }

    for batch in &frame.batches {
        let (lane, expected, both) = match batch.kind {
            BatchKind::Mono { page } => (LaneKind::Mono, [page, 0], false),
            BatchKind::Subpixel { page } => (LaneKind::Subpixel, [page, 0], false),
            BatchKind::Color { page } => (LaneKind::Color, [page, 0], false),
            BatchKind::ShaderQuads { shader, params } => {
                (LaneKind::ShaderQuads, [shader, params], true)
            }
            _ => continue,
        };
        for position in batch.range.clone() {
            let source = lane_sources(frame, lane)[position as usize];
            let key = keys[&(lane, source.index)];
            assert_eq!(key[0], expected[0], "a batch reads one binding");
            if both {
                assert_eq!(key[1], expected[1], "a batch reads one parameter slot");
            }
        }
    }
}
