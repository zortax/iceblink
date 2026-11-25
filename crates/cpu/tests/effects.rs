//! Kept vector rasters.

mod common;

use std::sync::Arc;

use iceblink::Renderer;
use iceblink::kurbo::{Affine, BezPath, Circle, Shape};
use iceblink::{FillRule, FramePath, PassKey, PathFill};

use common::build::{FrameBuilder, filled, whole};
use common::{cpu, draw};

fn stat(renderer: &iceblink_cpu::CpuRenderer, name: &str) -> u64 {
    let mut found = 0;
    renderer.extra_stats(&mut |stat, value| {
        if stat == name {
            found = value;
        }
    });
    found
}

#[test]
fn a_keyed_pass_drawn_again_is_composited_from_its_kept_raster() {
    let circle: Arc<BezPath> = Arc::new(Circle::new((40.0, 40.0), 20.0).to_path(0.1));
    let frame = || {
        let mut builder = FrameBuilder::new();
        let white = builder.solid([1.0; 4]);
        let green = builder.solid([0.0, 0.5, 0.0, 1.0]);
        builder.quad(filled([0.0, 0.0, 96.0, 96.0], white));
        let region = whole();
        let mut ink = region;
        ink.size.width = 64;
        builder.path(
            FramePath {
                path: Arc::clone(&circle),
                transform: Affine::IDENTITY,
                brush: Affine::IDENTITY,
                fill: Some(PathFill {
                    paint: green,
                    rule: FillRule::NON_ZERO,
                }),
                stroke: None,
                clips: Arc::from(Vec::new()),
                ink,
            },
            region,
            PassKey(7),
        );
        builder.finish()
    };
    let mut renderer = cpu();
    draw(&mut renderer, &frame());
    let first = renderer.composed().clone();
    draw(&mut renderer, &frame());
    assert_eq!(stat(&renderer, "vector_passes_rasterised"), 1);
    assert_eq!(stat(&renderer, "vector_passes_kept"), 1);
    assert_eq!(renderer.composed(), &first);
}
