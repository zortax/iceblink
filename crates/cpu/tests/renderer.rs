//! The renderer contract beyond the present suite: tables of an unconfigured frame, a composed
//! frame presented by the next one, a kept promise broken by a configure, and buffer age.

mod common;

use iceblink::Renderer;
use iceblink::damage::DamageSet;
use iceblink::{Acquired, BufferAge, Frame, FrameOutcome, RenderTarget, SkipReason};
use iceblink_cpu::{CpuRenderer, PixmapPresenter};

use common::build::{FrameBuilder, area, filled};
use common::fault::Faulty;
use common::{SIDE, configured, cpu};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

// A frame that fills `rect` of the output with `rgba`, over `damage`.
fn filling(rect: [f32; 4], rgba: [f32; 4], damage: Option<DamageSet>) -> Frame {
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(rgba);
    builder.quad(filled(rect, paint));
    if let Some(damage) = damage {
        builder.damage(damage);
    }
    builder.finish()
}

fn pixel(renderer: &CpuRenderer, x: u32, y: u32) -> [u8; 4] {
    renderer.composed().pixel(x, y).expect("inside the target")
}

fn faulty() -> CpuRenderer {
    configured(CpuRenderer::new(Box::new(Faulty::new(
        PixmapPresenter::kept([SIDE, SIDE]),
    ))))
}

fn faults(renderer: &mut CpuRenderer) -> &mut Faulty {
    renderer
        .presenter()
        .and_then(|presenter| presenter.as_any_mut())
        .and_then(|any| any.downcast_mut::<Faulty>())
        .expect("a faulty presenter")
}

#[test]
fn a_frame_before_configure_records_nothing_but_applies_its_tables() {
    let mut renderer = CpuRenderer::offscreen([SIDE, SIDE]);
    let first = filling([0.0, 0.0, 96.0, 96.0], RED, None);
    assert_eq!(
        renderer.draw(&first),
        FrameOutcome::Skipped(SkipReason::Unconfigured)
    );
    renderer.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    // The second frame names the paint slot the first one wrote, and writes no table.
    let mut second = filling([0.0, 0.0, 96.0, 96.0], BLUE, None);
    second.tables.paints.replace = false;
    second.tables.paints.all.clear();
    assert!(matches!(renderer.draw(&second), FrameOutcome::Presented(_)));
    assert_eq!(pixel(&renderer, 40, 40), [255, 0, 0, 255]);
}

#[test]
fn an_undamaged_frame_presents_what_was_composed_and_never_presented() {
    let mut renderer = faulty();
    faults(&mut renderer)
        .acquisitions
        .push_back(SkipReason::Unacquired);
    // Composed, and then no image: the damage retires with a timeout.
    assert_eq!(
        renderer.draw(&filling([0.0, 0.0, 96.0, 96.0], RED, None)),
        FrameOutcome::Skipped(SkipReason::Timeout)
    );
    let mut empty = filling([0.0, 0.0, 96.0, 96.0], RED, Some(DamageSet::new()));
    empty.tables = Default::default();
    assert!(matches!(renderer.draw(&empty), FrameOutcome::Presented(_)));
    assert_eq!(faults(&mut renderer).presented, 1);
    assert_eq!(
        renderer.draw(&empty),
        FrameOutcome::Skipped(SkipReason::Undamaged)
    );
}

#[test]
fn a_target_that_lost_its_picture_after_promising_it_skips_a_partial_frame() {
    let mut renderer = cpu();
    renderer.draw(&filling([0.0, 0.0, 96.0, 96.0], RED, None));
    assert_eq!(renderer.target_age(), BufferAge::Kept);
    // A new scale changes what every kept pixel means.
    renderer.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 2.0,
        opaque: true,
    });
    assert_eq!(
        renderer.draw(&filling(
            [0.0, 0.0, 96.0, 96.0],
            BLUE,
            Some(area(0, 0, 8, 8))
        )),
        FrameOutcome::Skipped(SkipReason::Unacquired)
    );
    assert_eq!(renderer.target_age(), BufferAge::Unknown);
}

#[test]
fn a_buffer_age_renderer_draws_into_the_image_acquired_before_the_build() {
    let mut renderer = configured(CpuRenderer::new(Box::new(PixmapPresenter::buffer_age(
        2,
        [SIDE, SIDE],
    ))));
    let frame = filling([0.0, 0.0, 96.0, 96.0], RED, None);
    assert_eq!(
        renderer.draw(&frame),
        FrameOutcome::Skipped(SkipReason::Unacquired)
    );
    assert!(!renderer.capabilities().can_shift);
    for expected in [BufferAge::Unknown, BufferAge::Unknown, BufferAge::Known(2)] {
        let presenter = renderer.presenter().expect("a presenter");
        let Acquired::Image { age, .. } = presenter.acquire() else {
            panic!("an image is free");
        };
        assert_eq!(age, expected);
        assert!(matches!(renderer.draw(&frame), FrameOutcome::Presented(_)));
    }
}
