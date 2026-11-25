//! Opening a device or refusing to, what a renderer reports about it, and what survives a loss.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, area, filled, frosted, opaque, rect};
use common::device::{SIDE, device_lock, draw, plain_renderer, present};
use common::present::target;
use iceblink::Renderer as _;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    BlendMode, FRAME_NO_CLIP, FrameFilter, FrameGroup, FrameOutcome, FramePath, MemoryReport,
    VectorBackend, VectorPlan, kurbo,
};
use iceblink_wgpu::vector::{
    Capacity, Layering, ResourcedPlan, VectorError, VectorFrame, VectorRaster, VectorTarget,
};
use iceblink_wgpu::{Builder, GroupPrecision, SrgbTier, wgpu};

/// Returns a frame of `side` filled with the opaque colour `color`.
fn square(side: u32, color: [u8; 3]) -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    builder.frame_mut().size = [side, side];
    let paint = builder.solid(opaque(color[0], color[1], color[2]));
    builder.quad(filled(rect(0.0, 0.0, side as f32, side as f32), paint));
    builder.finish()
}

#[test]
fn with_every_backend_masked_out_the_failure_names_no_candidate() {
    let _guard = device_lock();
    let failure = Builder::with_backends(wgpu::Backends::empty())
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Bgra8Unorm)
        .expect_err("no backend can produce a device");

    assert!(failure.candidates.is_empty(), "{:?}", failure.candidates);
}

#[test]
fn a_gl_device_that_opens_draws_and_one_that_does_not_names_its_reason() {
    let _guard = device_lock();
    match Builder::with_backends(wgpu::Backends::GL)
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm)
    {
        Ok(mut renderer) => {
            renderer.configure(target(SIDE));
            assert!(
                renderer
                    .draw(&square(SIDE, [0, 128, 255]))
                    .stats()
                    .is_some()
            );
            let pixels = renderer.read_presented().expect("an offscreen image");
            assert_eq!(pixels.rgba(32, 32), [0, 128, 255, 255], "a GL device draws");
        }
        Err(failure) => {
            for candidate in &failure.candidates {
                assert!(
                    !candidate.reason.is_empty(),
                    "{} names no reason",
                    candidate.name
                );
            }
            eprintln!("GL: {failure}");
        }
    }
}

#[test]
fn the_capabilities_and_formats_are_read_off_the_device() {
    let renderer = plain_renderer();

    assert!(renderer.capabilities().max_texture_size >= 2048);
    assert_eq!(
        renderer.capabilities().max_texture_size,
        renderer.gpu().capabilities().max_texture_size
    );
    let formats = renderer.formats();
    assert_eq!(formats.tier, SrgbTier::Native);
    assert!(!formats.surface.is_srgb() && !formats.scene.is_srgb());
    assert_eq!(formats.scratch, wgpu::TextureFormat::Rgba8Unorm);
    assert!(formats.is_sound());
}

#[test]
fn a_resize_inside_the_size_class_keeps_the_kept_target_and_one_past_it_still_draws() {
    let mut renderer = plain_renderer();
    let before = renderer.memory().targets;
    renderer.configure(target(SIDE + 8));

    assert_eq!(
        renderer.memory().targets,
        before,
        "the allocation is reused"
    );

    renderer.configure(target(300));
    assert!(renderer.memory().targets > before, "the allocation grows");

    let pixels = present(&mut renderer, &square(300, [255, 0, 0]));
    assert_eq!(pixels.size().width, 300);
    assert_eq!(pixels.rgba(299, 299), [255, 0, 0, 255], "the far corner");
}

#[test]
fn releasing_idle_resources_uploads_the_kept_tables_again() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let paint = first.solid(opaque(233, 71, 29));
    first.quad(filled(rect(8.0, 8.0, 48.0, 48.0), paint));

    assert_eq!(
        present(&mut renderer, &first.finish()).rgba(24, 24),
        [233, 71, 29, 255]
    );

    assert!(
        renderer.release_idle_resources() > 0,
        "something was released"
    );

    let mut next = FrameBuilder::next();
    next.quad(filled(rect(8.0, 8.0, 48.0, 48.0), paint));
    assert_eq!(
        present(&mut renderer, &next.finish()).rgba(24, 24),
        [233, 71, 29, 255],
        "the paint table was restored after its buffer was released"
    );
}

#[test]
fn an_empty_frame_clears_its_damage_and_copies_it() {
    let mut renderer = plain_renderer();
    let outcome = draw(&mut renderer, &FrameBuilder::new().finish());
    let stats = outcome.stats().expect("the frame was presented");

    assert_eq!(stats.draw_calls, 2, "the clear and the copy");
    let pixels = renderer.read_presented().expect("an offscreen image");
    assert_eq!(pixels.rgba(64, 64), [0, 0, 0, 0]);
}

#[test]
fn memory_is_reported_in_the_bytes_each_part_holds() {
    let mut renderer = plain_renderer();
    let memory = renderer.memory();
    // The kept target is allocated at a square size class, four bytes a texel.
    let texels = memory.targets / 4;
    let side = (texels as f64).sqrt() as u64;
    assert_eq!(memory.targets % 4, 0);
    assert!(
        side * side == texels && side >= u64::from(SIDE),
        "the kept target is held at four bytes a texel: {} bytes",
        memory.targets
    );
    assert!(memory.chunks > 0, "the frame buffers are held");

    present(&mut renderer, &square(SIDE, [0, 0, 0]));

    assert_eq!(
        renderer.memory().atlas,
        memory.atlas,
        "a frame without pages adds none"
    );
    assert_eq!(
        memory.total(),
        memory.fixed + memory.targets + memory.atlas + memory.chunks + memory.vector
    );
}

#[test]
fn a_small_renderer_reserves_small_memory_blocks() {
    let renderer = plain_renderer();
    let Some(report) = renderer.gpu().device().generate_allocator_report() else {
        assert_ne!(
            renderer.gpu().adapter().get_info().backend,
            wgpu::Backend::Vulkan,
            "a Vulkan device suballocates and reports its blocks"
        );
        return;
    };
    assert!(
        report.total_reserved_bytes <= 32 << 20,
        "{} bytes in {} blocks",
        report.total_reserved_bytes,
        report.blocks.len()
    );
}

#[test]
fn a_default_device_asks_for_no_timestamp_queries_and_times_nothing() {
    let mut renderer = plain_renderer();
    let asked = iceblink_wgpu::gpu_timing_requested();
    let timestamps = renderer
        .gpu()
        .device()
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY);

    assert_eq!(
        timestamps, asked,
        "timestamp queries are asked for only with timing on"
    );
    if asked {
        return;
    }
    assert!(!renderer.gpu_timing_enabled());
    assert_eq!(renderer.gpu_timed_frames(), None);
    present(&mut renderer, &square(SIDE, [0, 0, 0]));
    assert!(renderer.take_gpu_timings().is_empty());
}

#[test]
fn a_lost_device_is_rebuilt_and_the_frame_after_it_draws_the_same_pixels() {
    let mut renderer = plain_renderer();
    let frame = || {
        let mut builder = FrameBuilder::new();
        let paint = builder.solid(opaque(0, 200, 100));
        builder.quad(filled(rect(16.0, 16.0, 64.0, 64.0), paint));
        builder.finish()
    };
    let fresh = renderer.memory().atlas;
    let before = present(&mut renderer, &frame());

    renderer
        .gpu()
        .loss()
        .report(wgpu::DeviceLostReason::Unknown, "injected by a test");

    assert_eq!(draw(&mut renderer, &frame()), FrameOutcome::Recovered);
    assert!(
        !renderer.gpu().loss().is_lost(),
        "the rebuilt device is a new one"
    );
    assert_eq!(
        renderer.memory().atlas,
        fresh,
        "nothing the old device held survives"
    );

    let after = present(&mut renderer, &frame());
    assert_eq!(before.max_difference(&after), 0);
}

/// A rasteriser that runs out of capacity before it draws a pass.
struct Short;

impl VectorRaster for Short {
    fn plan(&mut self, plan: &VectorPlan) -> ResourcedPlan {
        let regions: Vec<_> = plan.passes.iter().map(|pass| pass.region).collect();
        ResourcedPlan::placed(plan, &Layering::of(&regions, Capacity::DEFAULT))
    }

    fn clear_targets(&mut self, _plan: &ResourcedPlan, _round: usize) {}

    fn prepare(&mut self, _frame: &mut VectorFrame<'_>) -> Result<(), VectorError> {
        Err(VectorError::OutOfCapacity {
            detail: "a test rasteriser holds nothing".into(),
            prepared: 0,
        })
    }

    fn view(&self, _target: VectorTarget) -> Option<&wgpu::TextureView> {
        None
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport::default()
    }
}

#[test]
fn every_setting_survives_a_lost_device() {
    let budget = 1 << 20;
    let mut renderer = plain_renderer();
    renderer.set_group_precision(GroupPrecision::High);
    renderer.set_group_budget(budget);
    renderer.set_target_margin(6);
    renderer.set_rasters_kept(false);
    renderer.set_vector_shortfall_fatal(false);
    renderer.set_vector_factory(
        Box::new(|_, _| Box::new(Short)),
        VectorBackend::Other,
        "short",
    );
    let covered = renderer.groups().covered();

    renderer
        .gpu()
        .loss()
        .report(wgpu::DeviceLostReason::Unknown, "injected by a test");
    let recovered = draw(&mut renderer, &FrameBuilder::new().finish());

    assert_eq!(recovered, FrameOutcome::Recovered);
    assert_eq!(renderer.group_precision(), GroupPrecision::High);
    assert_eq!(renderer.groups().budget(), budget);
    assert_eq!(renderer.groups().covered(), covered, "the margin");
    assert!(!renderer.capabilities().kept_rasters);
    let mut builder = FrameBuilder::new();
    builder.path(
        FramePath {
            path: Arc::new(kurbo::BezPath::new()),
            transform: kurbo::Affine::IDENTITY,
            brush: kurbo::Affine::IDENTITY,
            fill: None,
            stroke: None,
            clips: Arc::from(Vec::new()),
            ink: Rect::new(Point::new(16, 16), Size::new(32, 32)),
        },
        FRAME_NO_CLIP,
    );
    assert!(
        draw(&mut renderer, &builder.finish()).stats().is_some(),
        "a shortfall that is not fatal draws the frame"
    );
}

#[test]
fn two_hundred_frames_of_nested_groups_and_backdrops_hold_the_memory_of_five() {
    let mut renderer = plain_renderer();
    let frame = |step: u32| {
        let mut builder = FrameBuilder::new();
        let white = builder.solid(opaque(255, 255, 255));
        let paint = builder.solid(opaque((step * 7 % 256) as u8, 40, 90));
        builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), white));
        for level in 0..4 {
            builder.begin_group(FrameGroup {
                bounds: rect(8.0 + level as f32, 8.0, 100.0, 100.0),
                source: rect(8.0 + level as f32, 8.0, 100.0, 100.0),
                clip: FRAME_NO_CLIP,
                opacity: 0.7,
                blend: BlendMode::NORMAL,
                filters: Arc::from(vec![FrameFilter::Blur(3.0)]),
            });
        }
        builder.quad(filled(rect(24.0, 24.0, 60.0, 60.0), paint));
        for _ in 0..4 {
            builder.end_group();
        }
        builder.backdrop(frosted(rect(20.0, 20.0, 60.0, 60.0), 5.0));
        builder.finish()
    };

    for step in 0..5 {
        draw(&mut renderer, &frame(step));
    }

    let held = |memory: iceblink::MemoryReport| memory.targets + memory.atlas;
    let settled = held(renderer.memory());
    assert!(
        renderer.groups().peak() >= 4,
        "the pool lent {}",
        renderer.groups().peak()
    );

    for step in 5..200 {
        draw(&mut renderer, &frame(step));
    }

    assert!(
        held(renderer.memory()) <= settled,
        "later frames allocated nothing"
    );
    assert_eq!(renderer.groups().lent(), 0, "every lease came back");
}

#[test]
fn sibling_backdrops_return_their_captures_before_the_next_one_is_taken() {
    let mut renderer = plain_renderer();
    // Four full-output targets at eight bytes a texel.
    renderer.set_group_budget(4 * 8 * u64::from(SIDE * SIDE) * 4);
    let mut builder = FrameBuilder::new();
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), white));
    for index in 0..12 {
        let (column, row) = ((index % 4) as f32, (index / 4) as f32);
        builder.backdrop(frosted(
            rect(4.0 + column * 30.0, 4.0 + row * 40.0, 24.0, 24.0),
            2.0,
        ));
    }

    present(&mut renderer, &builder.finish());

    assert!(
        renderer.groups().peak() <= 3,
        "a capture and two blur targets serve one backdrop, and the pool lent {}",
        renderer.groups().peak()
    );
    assert_eq!(renderer.groups().degraded(), 0, "no lease was halved");
    assert_eq!(renderer.groups().lent(), 0);
}

#[test]
fn a_frame_culled_for_a_kept_target_that_must_be_redrawn_whole_keeps_its_damage() {
    let mut renderer = plain_renderer();
    present(&mut renderer, &square(SIDE, [0, 0, 0]));
    assert_eq!(renderer.target_age(), iceblink::BufferAge::Kept);

    renderer.set_group_precision(GroupPrecision::High);
    let mut builder = FrameBuilder::next().id(2);
    let paint = builder.solid(opaque(200, 0, 0));
    builder.quad(filled(rect(8.0, 8.0, 16.0, 16.0), paint));
    let frame = builder.damage(area(8, 8, 16, 16)).finish();

    let outcome = draw(&mut renderer, &frame);

    assert_eq!(
        outcome,
        FrameOutcome::Skipped(iceblink::SkipReason::Unacquired)
    );
    assert!(!outcome.retires_damage());
    assert_eq!(renderer.target_age(), iceblink::BufferAge::Unknown);
}
