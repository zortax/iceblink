//! Path routes: when a small solid path that changes in most frames becomes a frame path.

use std::f64::consts::TAU;
use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Rect, Shape};
use iceblink::{
    Built, ChunkRecorder, ClipRef, FillRule, OrderEntry, PaintRef, PathFill, PathItem, SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{device, key, quad};
use crate::conformance::scene::stage::{Stage, covers};

/// How a frame drew a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Masks,
    Path,
}

/// A path of more than 4096 segments that changed in three of four frames becomes a frame path,
/// and keeps that route while it stays the same. Eight frames after its masks last differed, it
/// takes masks again in a frame of its own, which the idle frames before it ask for.
pub fn path_routes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let outline = Arc::new(polygon(4200));

    // Each step moves the path by a quarter pixel or keeps it, and repaints a quad under it so
    // the frame draws it either way.
    let shifts = [0.0, 0.25, 0.5, 0.75].into_iter().chain([0.75; 3]);
    let routes: Vec<Route> = shifts
        .enumerate()
        .map(|(step, shift)| draw_polygon(&mut stage, &outline, shift, step as u8))
        .collect();
    let mut expected = vec![Route::Masks; 3];
    expected.extend([Route::Path; 4]);
    assert_eq!(
        routes, expected,
        "a volatile path of 4200 segments becomes a frame path while it stays the same"
    );

    for frame in 4..8 {
        assert!(
            matches!(stage.draw(), Built::Idle { wake_ns: Some(_) }),
            "frame {frame} after the path's masks last differed builds nothing and asks for the \
             next one"
        );
    }
    assert_eq!(
        stage.draw(),
        Built::Frame,
        "eight frames after the path's masks last differed, it takes masks again"
    );
    assert_eq!(drawn(&stage), (1, 0), "the path is drawn with masks");
    assert!(
        covers(stage.damage(), device(20, 20, 25, 24)),
        "the frame damages the path's ink, but damages {:?}",
        stage.damage()
    );
    assert_eq!(
        stage.draw(),
        Built::Idle { wake_ns: None },
        "once the path took masks again, nothing asks for a frame"
    );
}

/// A path item that its chunk no longer holds loses its route history, so a path recorded at its
/// position again takes masks.
pub fn removed_path_routes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let outline = Arc::new(polygon(4200));
    for (step, shift) in [0.0, 0.25, 0.5, 0.75].into_iter().enumerate() {
        draw_polygon(&mut stage, &outline, shift, step as u8);
    }

    commit(&mut stage, false, |chunk| {
        chunk.quad(quad([0.0, 0.0, 64.0, 64.0], [4, 0, 0, 255]));
    });
    stage.draw();

    assert_eq!(
        draw_polygon(&mut stage, &outline, 0.75, 5),
        Route::Masks,
        "a frame path recorded again after its chunk dropped it takes masks"
    );
}

/// A `build_frame` call that builds no frame still counts as a frame of the route history.
pub fn idle_route_frames(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let outline = Arc::new(polygon(4200));
    for (step, shift) in [0.0, 0.25, 0.5].into_iter().enumerate() {
        draw_polygon(&mut stage, &outline, shift, step as u8);
    }
    for _ in 0..2 {
        assert!(
            matches!(stage.draw(), Built::Idle { .. }),
            "nothing changed, so no frame is built"
        );
    }

    assert_eq!(
        draw_polygon(&mut stage, &outline, 0.75, 3),
        Route::Masks,
        "a path that changed in two of the last four frames, two of them idle, keeps its masks"
    );
}

/// A frame admits 256 new masks of volatile paths, in the order it draws them, and the paths past
/// that become frame paths.
pub fn route_budget(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let corner = |index: usize| {
        [
            4.0 + 7.0 * (index % 17) as f64,
            4.0 + 7.0 * (index / 17) as f64,
        ]
    };
    let squares: Vec<Arc<BezPath>> = (0..257)
        .map(|index| {
            let [x, y] = corner(index);
            Arc::new(Rect::new(x, y, x + 3.0, y + 3.0).to_path(0.1))
        })
        .collect();

    // Every path moves by a quarter pixel in every step, and turns volatile in the fourth.
    for step in 0..4 {
        let shift = 0.25 * f64::from(step);
        commit(&mut stage, step == 0, |chunk| {
            for square in &squares {
                chunk.path(filled(square, shift));
            }
        });
        stage.draw();
    }

    let (sprites, paths) = drawn(&stage);
    assert_eq!(
        (sprites, paths),
        (256, 1),
        "256 volatile paths get new masks, and the 257th becomes a frame path"
    );
    let [x, y] = corner(256);
    let ink = stage.frame.paths[stage.frame.vector_plan.items[0].path as usize].ink;
    assert_eq!(
        [ink.origin.x, ink.origin.y],
        [x as i32, y as i32],
        "the path drawn last is the one past the budget"
    );
}

/// Records the polygon moved right by `shift` over a quad of red `shade`, draws a frame and
/// returns how it drew the polygon.
///
/// The chunk is appended to the root at shade zero.
fn draw_polygon(stage: &mut Stage, outline: &Arc<BezPath>, shift: f64, shade: u8) -> Route {
    commit(stage, shade == 0, |chunk| {
        chunk.quad(quad([0.0, 0.0, 64.0, 64.0], [shade, 0, 0, 255]));
        chunk.path(filled(outline, shift));
    });
    stage.draw();

    match drawn(stage) {
        (1, 0) => Route::Masks,
        (0, 1) => Route::Path,
        (sprites, paths) => {
            panic!("the path is drawn by {sprites} sprites and {paths} frame paths")
        }
    }
}

/// Records chunk 1 with what `record` adds and commits it, appending it to the root when `first`.
fn commit(stage: &mut Stage, first: bool, record: impl FnOnce(&mut ChunkRecorder<'_>)) {
    let mut tx = stage.tx();
    record(&mut tx.record(key(1), stage.props()));
    if first {
        stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    }
    stage.commit(tx);
}

/// Returns how many sprites and frame paths the last frame drew.
fn drawn(stage: &Stage) -> (usize, usize) {
    (
        stage.picture.sprites.len(),
        stage.frame.vector_plan.items.len(),
    )
}

/// Returns a path item filling `outline` in black, moved right by `shift`.
fn filled(outline: &Arc<BezPath>, shift: f64) -> PathItem {
    PathItem {
        path: Arc::clone(outline),
        placement: Affine::translate((shift, 0.0)),
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint: PaintRef::color([0, 0, 0, 255]),
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from([]),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    }
}

/// Returns a closed polygon of `corners` corners on a circle of radius 12 around `(32, 32)`.
fn polygon(corners: usize) -> BezPath {
    let corner = |index: usize| {
        let angle = TAU * index as f64 / corners as f64;
        (32.0 + 12.0 * angle.cos(), 32.0 + 12.0 * angle.sin())
    };

    let mut outline = BezPath::new();
    outline.move_to(corner(0));
    for index in 1..corners {
        outline.line_to(corner(index));
    }
    outline.close_path();
    outline
}
