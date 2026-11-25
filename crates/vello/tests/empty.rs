//! A frame without paths.

mod common;

use iceblink::VectorPlan;
use iceblink_wgpu::vector::VectorRaster;

use common::raster::{coverage, vello};
use iceblink_testing::vector::open_device;

#[test]
fn a_frame_without_passes_runs_no_vector_work() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut by_vello = vello(&gpu);
    let mut by_coverage = coverage(&gpu, 64);
    let scratch = by_coverage.scratch_bytes();

    let one = by_vello.plan(&VectorPlan::default());
    let two = by_coverage.plan(&VectorPlan::default());

    assert!(one.is_empty() && two.is_empty());
    assert_eq!((one.rounds(), two.rounds()), (0, 0));
    assert_eq!(by_vello.memory().vector, 0, "no scratch was allocated");
    assert_eq!(by_vello.renders(), 0);
    assert_eq!(by_coverage.scratch_bytes(), scratch);
}
