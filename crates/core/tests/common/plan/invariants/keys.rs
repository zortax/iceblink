//! Where a plan draws each logged entry.

use std::collections::HashMap;

use iceblink::{BatchKind, Frame};
use iceblink_core::plan::LaneKind;

use crate::common::plan::scene::{Pushed, Scene, lane_sources};

use super::DrawKey;

/// Returns the draw key of every logged entry that was drawn, by log position.
pub(super) fn draw_keys(scene: &Scene, frame: &Frame) -> Vec<Option<DrawKey>> {
    let Batches {
        lanes: lane_batches,
        singles,
        paths,
    } = batches(frame);
    let path_pass = path_passes(frame);
    let positions = lane_positions(frame);

    scene
        .log
        .iter()
        .map(|logged| match logged.pushed {
            Pushed::Instance { lane, index, .. } => positions.get(&(lane, index)).map(|position| {
                let batch = lane_batches[&lane][*position as usize].expect("drawn");
                (batch, *position)
            }),
            Pushed::Path { index, .. } => path_pass.get(&index).map(|pass| (paths[pass], 0)),
            Pushed::External { index } => singles.get(&(0, index)).map(|batch| (*batch, 0)),
            Pushed::Backdrop { index } => singles.get(&(1, index)).map(|batch| (*batch, 0)),
            Pushed::GroupBegin { index } => singles.get(&(2, index)).map(|batch| (*batch, 0)),
            Pushed::GroupEnd { index } => singles.get(&(3, index)).map(|batch| (*batch, 0)),
        })
        .collect()
}

/// The batch that draws each position of each lane, pass and single entry.
struct Batches {
    /// The batch of each position of each lane.
    lanes: HashMap<LaneKind, Vec<Option<u32>>>,
    /// The batch of each external quad, backdrop and group marker, by kind and index.
    singles: HashMap<(u8, u32), u32>,
    /// The batch of each vector pass.
    paths: HashMap<u32, u32>,
}

/// Returns the batch that draws every lane position, pass and single entry of `frame`.
///
/// Panics when a batch is malformed, a lane position is drawn twice or not at all, or a pass has
/// no batch or two.
fn batches(frame: &Frame) -> Batches {
    let mut lane_batches: HashMap<LaneKind, Vec<Option<u32>>> = HashMap::new();
    let mut singles: HashMap<(u8, u32), u32> = HashMap::new();
    let mut paths: HashMap<u32, u32> = HashMap::new();
    for lane in LaneKind::ALL {
        lane_batches.insert(lane, vec![None; lane_sources(frame, lane).len()]);
    }

    for (index, batch) in frame.batches.iter().enumerate() {
        let index = index as u32;
        let lane = match batch.kind {
            BatchKind::Quads => Some(LaneKind::Quads),
            BatchKind::ShaderQuads { .. } => Some(LaneKind::ShaderQuads),
            BatchKind::Shadows => Some(LaneKind::Shadows),
            BatchKind::Lines => Some(LaneKind::Lines),
            BatchKind::Mono { .. } => Some(LaneKind::Mono),
            BatchKind::Subpixel { .. } => Some(LaneKind::Subpixel),
            BatchKind::Color { .. } => Some(LaneKind::Color),
            BatchKind::Marks => Some(LaneKind::Marks),
            BatchKind::Paths { pass } => {
                assert!(
                    paths.insert(pass, index).is_none(),
                    "two batches for pass {pass}"
                );
                None
            }
            BatchKind::External(at) => {
                assert!(singles.insert((0, at), index).is_none());
                None
            }
            BatchKind::Backdrop(at) => {
                assert!(singles.insert((1, at), index).is_none());
                None
            }
            BatchKind::GroupBegin(at) => {
                assert!(singles.insert((2, at), index).is_none());
                None
            }
            BatchKind::GroupEnd(at) => {
                assert!(singles.insert((3, at), index).is_none());
                None
            }
        };
        match lane {
            Some(lane) => {
                assert!(
                    !batch.range.is_empty(),
                    "an instanced batch draws something"
                );
                let held = lane_batches.get_mut(&lane).expect("every lane is listed");
                for position in batch.range.clone() {
                    let slot = held
                        .get_mut(position as usize)
                        .expect("a batch range lies inside its lane");
                    assert!(
                        slot.is_none(),
                        "position {position} of {lane:?} is drawn twice"
                    );
                    *slot = Some(index);
                }
            }
            None => assert!(batch.range.is_empty(), "a single batch has an empty range"),
        }
    }

    for lane in LaneKind::ALL {
        assert!(
            lane_batches[&lane].iter().all(Option::is_some),
            "every source of {lane:?} is drawn by a batch"
        );
    }
    assert_eq!(
        paths.len(),
        frame.vector_plan.passes.len(),
        "one batch per pass"
    );

    Batches {
        lanes: lane_batches,
        singles,
        paths,
    }
}

/// Returns the pass of every path in the vector plan of `frame`.
fn path_passes(frame: &Frame) -> HashMap<u32, u32> {
    let mut path_pass: HashMap<u32, u32> = HashMap::new();
    for (pass_index, pass) in frame.vector_plan.passes.iter().enumerate() {
        for item in frame.vector_plan.items_of(pass) {
            assert!(
                path_pass.insert(item.path, pass_index as u32).is_none(),
                "path {} is in two passes",
                item.path
            );
        }
    }
    path_pass
}

/// Returns the position of every transient instance in its lane, by lane and transient index.
fn lane_positions(frame: &Frame) -> HashMap<(LaneKind, u32), u32> {
    let mut positions: HashMap<(LaneKind, u32), u32> = HashMap::new();
    for lane in LaneKind::ALL {
        for (position, source) in lane_sources(frame, lane).iter().enumerate() {
            assert!(source.is_transient());
            assert!(
                positions
                    .insert((lane, source.index), position as u32)
                    .is_none()
            );
        }
    }
    positions
}
