//! How much scratch a frame takes, and how a frame larger than its scratch is drawn whole.

mod common;

use iceblink_wgpu::vector::Capacity;

use common::raster::{Which, raster, vello};
use iceblink_testing::vector::draw::{draw, draw_twice};
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, rect};

/// The side of the output.
const SIDE: i32 = 640;

/// A scratch so small that a few hundred passes take dozens of rounds.
const SMALL: Capacity = Capacity {
    side: 64,
    layers: 2,
};

/// How many steps the staircase has.
const STEPS: u32 = 160;

/// Where square `index` of a grid of 8 by 8 squares, 16 pixels apart, is drawn.
fn cell(index: u32) -> (f64, f64) {
    (
        16.0 + f64::from(index % 39) * 16.0,
        16.0 + f64::from(index / 39) * 16.0,
    )
}

/// A frame of `count` white squares that nowhere touch, each in a pass of its own.
fn one_pass_each(count: u32) -> Scene {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    for index in 0..count {
        let (x, y) = cell(index);
        let path = scene.fill(rect(x, y, 8.0, 8.0), white);
        scene.pass(&[path]);
    }
    scene
}

/// A frame of `count` squares stacked on one spot, each in a pass of its own, the last one red.
fn one_stack(count: u32) -> Scene {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let red = scene.solid([255, 0, 0, 255]);
    for index in 0..count {
        let paint = if index + 1 == count { red } else { white };
        let path = scene.fill(rect(16.0, 16.0, 24.0, 24.0), paint);
        scene.pass(&[path]);
    }
    scene
}

/// The colour of step `index` of a staircase in `shade`.
fn step_colour(index: u32, shade: u8) -> [u8; 4] {
    [index as u8, 255 - index as u8, shade, 255]
}

/// The left edge of step `index`.
fn step_left(index: u32) -> f64 {
    16.0 + 2.0 * f64::from(index)
}

/// A staircase of squares, each two pixels right of the one before and in a pass of its own.
///
/// The strip at the left edge of each step shows its colour only when every pass is drawn, in
/// order.
fn staircase(shade: u8) -> Scene {
    let mut scene = Scene::new(SIDE);
    for index in 0..STEPS {
        let paint = scene.solid(step_colour(index, shade));
        let path = scene.fill(rect(step_left(index), 16.0, 24.0, 24.0), paint);
        scene.pass(&[path]);
    }
    scene
}

fn assert_steps_in_order(
    canvas: &iceblink_testing::vector::canvas::Canvas,
    shade: u8,
    label: &str,
) {
    for index in 0..STEPS {
        let got = canvas.rgba(step_left(index) as i32, 28);
        let want = step_colour(index, shade);
        let close = (0..4).all(|channel| got[channel].abs_diff(want[channel]) <= 2);
        assert!(
            close,
            "{label}: step {index} reads {got:?} where {want:?} belongs"
        );
    }
}

#[test]
fn two_disjoint_passes_share_a_layer_and_composite_in_order() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let red = scene.solid([255, 0, 0, 255]);
    let blue = scene.solid([0, 0, 255, 255]);
    let left = scene.fill(rect(16.0, 16.0, 32.0, 32.0), red);
    let right = scene.fill(rect(400.0, 300.0, 32.0, 32.0), blue);
    scene.pass(&[left]).pass(&[right]);
    let mut raster = vello(&gpu);

    let drawn = draw(&gpu, &mut raster, &scene);

    drawn.outcome.expect("both passes fit");
    assert_eq!(raster.depth(), 1);
    assert_eq!(drawn.plan.passes[0].target, drawn.plan.passes[1].target);
    assert_eq!(drawn.canvas.rgba(32, 32), [255, 0, 0, 255]);
    assert_eq!(drawn.canvas.rgba(416, 316), [0, 0, 255, 255]);
}

#[test]
fn two_overlapping_passes_share_a_layer_in_bins_of_their_own() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let red = scene.solid([255, 0, 0, 255]);
    let blue = scene.solid([0, 0, 255, 255]);
    let under = scene.fill(rect(16.0, 16.0, 32.0, 32.0), red);
    let over = scene.fill(rect(32.0, 32.0, 32.0, 32.0), blue);
    scene.pass(&[under]).pass(&[over]);
    let mut raster = vello(&gpu);

    let drawn = draw(&gpu, &mut raster, &scene);

    let (one, two) = (&drawn.plan.passes[0], &drawn.plan.passes[1]);
    assert_eq!(one.target, two.target);
    assert!(
        one.raster_region.intersection(two.raster_region).is_none(),
        "the bins meet: {:?} and {:?}",
        one.raster_region,
        two.raster_region
    );
    assert_eq!(drawn.canvas.rgba(24, 24), [255, 0, 0, 255]);
    assert_eq!(
        drawn.canvas.rgba(40, 40),
        [0, 0, 255, 255],
        "the later pass is on top"
    );
}

#[test]
fn a_frame_of_five_hundred_passes_draws_all_of_them_in_one_layer() {
    let Some(gpu) = open_device() else {
        return;
    };
    let count = 500;
    let scene = one_pass_each(count);
    let mut raster = vello(&gpu);

    let drawn = draw(&gpu, &mut raster, &scene);

    drawn.outcome.expect("five hundred small passes fit");
    assert_eq!(raster.depth(), 1);
    for index in 0..count {
        let (x, y) = cell(index);
        assert_eq!(
            drawn.canvas.rgba(x as i32 + 4, y as i32 + 4),
            [255; 4],
            "square {index}"
        );
    }
}

#[test]
fn a_stack_deeper_than_the_most_layers_draws_every_pass() {
    let Some(gpu) = open_device() else {
        return;
    };
    let scene = one_stack(64 * 4);
    let mut raster = vello(&gpu);

    let drawn = draw(&gpu, &mut raster, &scene);

    drawn.outcome.expect("every pass has a bin");
    assert_eq!((raster.depth(), raster.rounds()), (1, 1));
    assert_eq!(
        drawn.canvas.rgba(24, 24),
        [255, 0, 0, 255],
        "the last pass is drawn on top"
    );
}

#[test]
fn a_frame_larger_than_its_scratch_draws_every_pass_in_order() {
    let Some(gpu) = open_device() else {
        return;
    };
    let scene = staircase(64);
    for which in [Which::Vello, Which::Coverage] {
        let mut whole = raster(&gpu, which, SIDE, Capacity::DEFAULT);
        let mut small = raster(&gpu, which, SIDE, SMALL);

        let one_round = draw(&gpu, whole.as_mut(), &scene);
        let rounds = draw(&gpu, small.as_mut(), &scene);

        assert_eq!(one_round.plan.rounds(), 1);
        assert!(
            rounds.plan.rounds() > 20,
            "{which:?}: {} rounds",
            rounds.plan.rounds()
        );
        rounds.outcome.expect("every round fits");
        assert_steps_in_order(&one_round.canvas, 64, &format!("{which:?} in one round"));
        assert_steps_in_order(&rounds.canvas, 64, &format!("{which:?} in rounds"));
        assert_eq!(
            rounds.canvas.difference(&one_round.canvas),
            None,
            "{which:?}"
        );
    }
}

#[test]
fn rounds_prepared_again_for_a_second_rectangle_draw_the_same_picture() {
    let Some(gpu) = open_device() else {
        return;
    };
    let scene = staircase(192);
    for which in [Which::Vello, Which::Coverage] {
        let mut once = raster(&gpu, which, SIDE, SMALL);
        let mut twice = raster(&gpu, which, SIDE, SMALL);

        let single = draw(&gpu, once.as_mut(), &scene);
        let repeated = draw_twice(&gpu, twice.as_mut(), &scene);

        repeated.outcome.expect("every round fits");
        assert_eq!(
            repeated.canvas.difference(&single.canvas),
            None,
            "{which:?}"
        );
    }
}

#[test]
fn a_frame_that_prepares_its_rounds_twice_reports_the_whole_frame() {
    let Some(gpu) = open_device() else {
        return;
    };
    let scene = staircase(64);
    let mut raster = vello(&gpu).with_capacity(SMALL);

    draw_twice(&gpu, &mut raster, &scene);

    assert_eq!(
        raster.passes(),
        2 * STEPS,
        "every preparation of every round counts"
    );
}
