//! Asking a device for Vulkan device extensions wgpu does not enable by itself.
//!
//! The list is all or nothing: a device enables every name an adapter grants or none, a device
//! that cannot grant the list still opens and draws, and the device that replaces a lost one asks
//! for the same list.

mod common;

use std::ffi::CStr;
use std::sync::{Arc, MutexGuard};

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, device_lock};
use common::present::Rig;
use iceblink::Renderer as _;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{AtlasTile, Color, FRAME_NO_CLIP, FrameOutcome, PageFormat, PageOp, Sprite};
use iceblink_wgpu::{Gpu, SharedGraphics, wgpu};

/// The extensions a scanout buffer exported as a dma-buf needs.
const DMA_BUF: [&CStr; 5] = [
    c"VK_EXT_image_drm_format_modifier",
    c"VK_KHR_external_memory_fd",
    c"VK_EXT_external_memory_dma_buf",
    c"VK_KHR_external_semaphore_fd",
    c"VK_EXT_queue_family_foreign",
];

/// An extension name no driver has.
// No registered extension carries this author tag.
const ABSENT: &CStr = c"VK_ICEBLINK_not_a_real_extension";

/// The extensions a device enables for the native presenter where the adapter grants them.
const PRESENTER: [&CStr; 4] = [
    c"VK_KHR_incremental_present",
    c"VK_GOOGLE_display_timing",
    c"VK_KHR_present_id",
    c"VK_KHR_present_wait",
];

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// Returns graphics on the default backends that ask devices for `extensions`.
fn graphics(extensions: &[&'static CStr]) -> SharedGraphics {
    SharedGraphics::with_backends_and_extensions(
        SharedGraphics::new().backends(),
        extensions.to_vec(),
    )
}

/// Opens the device of `graphics`.
fn open(graphics: &SharedGraphics) -> Arc<Gpu> {
    graphics
        .open_gpu()
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"))
}

/// Returns the extensions `gpu` was asked for and enabled, without the presenter's own.
fn enabled(gpu: &Gpu) -> Vec<&'static CStr> {
    gpu.vulkan_extensions()
        .iter()
        .copied()
        .filter(|name| !PRESENTER.contains(name))
        .collect()
}

/// Returns a rig over an offscreen renderer from `graphics` that holds `guard`.
fn rig(graphics: &SharedGraphics, guard: MutexGuard<'static, ()>) -> Rig {
    let renderer = graphics
        .renderer_offscreen([SIDE, SIDE], FORMAT)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    Rig::new(renderer, guard)
}

/// Asserts that `rig` draws a frame filled with `color`.
fn draws(rig: &mut Rig, color: [u8; 3]) {
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(opaque(color[0], color[1], color[2]));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), paint));
    let pixels = rig.present(&builder.finish());
    assert_eq!(
        pixels.rgba(SIDE as i32 / 2, SIDE as i32 / 2),
        [color[0], color[1], color[2], 255],
        "the device draws"
    );
}

#[test]
fn a_name_no_driver_has_leaves_the_device_open_with_nothing_enabled() {
    let guard = device_lock();
    let graphics = graphics(&[ABSENT]);
    assert_eq!(graphics.extensions(), [ABSENT]);
    assert!(enabled(&open(&graphics)).is_empty());

    draws(&mut rig(&graphics, guard), [255, 0, 0]);
}

#[test]
fn one_absent_name_costs_the_whole_list() {
    let _guard = device_lock();
    let mut mixed = DMA_BUF.to_vec();
    mixed[2] = ABSENT;
    let gpu = open(&graphics(&mixed));

    assert!(
        enabled(&gpu).is_empty(),
        "part of a list was enabled: {:?}",
        gpu.vulkan_extensions()
    );
}

#[test]
fn the_names_a_scanout_buffer_needs_are_enabled_where_the_driver_has_them() {
    let guard = device_lock();
    let graphics = graphics(&DMA_BUF);
    let gpu = open(&graphics);

    assert_eq!(
        gpu.adapter().get_info().backend,
        wgpu::Backend::Vulkan,
        "{} is not a Vulkan device",
        gpu.describe()
    );
    assert_eq!(
        enabled(&gpu),
        DMA_BUF.as_slice(),
        "{} grants every name of the list",
        gpu.describe()
    );

    draws(&mut rig(&graphics, guard), [0, 128, 255]);
}

#[test]
fn an_empty_list_opens_the_device_it_always_did() {
    let guard = device_lock();
    let graphics = SharedGraphics::new();
    assert!(graphics.extensions().is_empty());
    assert!(enabled(&open(&graphics)).is_empty());
    draws(&mut rig(&graphics, guard), [0, 255, 0]);
}

#[test]
fn a_device_opened_with_the_list_blends_against_a_second_colour_output() {
    let guard = device_lock();
    let graphics = graphics(&DMA_BUF);
    let mut rig = rig(&graphics, guard);
    assert!(!enabled(rig.gpu()).is_empty(), "the list was granted");
    assert!(
        rig.capabilities().subpixel_text,
        "{} blends against a second colour output",
        rig.gpu().describe()
    );

    const TILE: u32 = 16;
    let mut builder = FrameBuilder::new();
    builder.page(PageOp::Create {
        page: 0,
        format: PageFormat::SUBPIXEL,
        size: [TILE, TILE],
        levels: 1,
    });
    let mut bytes = Vec::with_capacity((TILE * TILE * 4) as usize);
    for _ in 0..TILE {
        for x in 0..TILE {
            let base = (255 * x / (TILE - 1)) as u8;
            bytes.extend_from_slice(&[base, base.saturating_add(40), base.saturating_add(80), 255]);
        }
    }
    builder.page(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(TILE as i32, TILE as i32)),
        bytes: Arc::from(bytes),
        offset: 0,
        stride: TILE * 4,
    });
    let white = builder.solid(opaque(255, 255, 255));
    let black = builder.solid(Color([0.0, 0.0, 0.0, 1.0]));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), white));
    builder.subpixel(Sprite {
        bounds: rect(0.0, 0.0, TILE as f32, TILE as f32),
        paint: black,
        paint_origin: [0.0; 2],
        tile: AtlasTile {
            page: 0,
            rect: [0, 0, TILE, TILE],
        },
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    });

    let sample = rig
        .present(&builder.finish())
        .rgba(TILE as i32 / 2, TILE as i32 / 2);
    assert_ne!(
        sample[0], sample[2],
        "per-channel coverage was drawn: {sample:?}"
    );
}

#[test]
fn an_adapter_that_grants_the_list_is_preferred_over_one_that_does_not() {
    let _guard = device_lock();
    let graphics = graphics(&DMA_BUF);
    let gpu = open(&graphics);
    if !enabled(&gpu).is_empty() {
        return;
    }
    // The device has none of them, so no adapter may grant the list.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: graphics.backends(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    for candidate in futures::executor::block_on(instance.enumerate_adapters(graphics.backends())) {
        let name = candidate.get_info().name;
        if let Ok(other) = Gpu::open(instance.clone(), candidate, &DMA_BUF) {
            assert!(
                enabled(&other).is_empty(),
                "{name} grants the list and another device was chosen"
            );
        }
    }
}

#[test]
fn enabling_the_list_changes_no_feature_limit_or_capability() {
    let _guard = device_lock();
    let (features, limits, capabilities) = {
        let gpu = open(&SharedGraphics::new());
        (
            gpu.device().features(),
            gpu.device().limits(),
            gpu.capabilities(),
        )
    };

    let gpu = open(&graphics(&DMA_BUF));

    assert!(!enabled(&gpu).is_empty(), "the list was granted");
    assert_eq!(gpu.device().features(), features);
    assert_eq!(gpu.device().limits(), limits);
    assert_eq!(gpu.capabilities(), capabilities);
}

#[test]
fn the_device_that_replaces_a_lost_one_enables_what_the_lost_one_did() {
    let guard = device_lock();
    let graphics = graphics(&DMA_BUF);
    let mut rig = rig(&graphics, guard);
    let before = rig.gpu().vulkan_extensions().to_vec();
    let dead = Arc::as_ptr(rig.gpu());

    rig.gpu()
        .loss()
        .report(wgpu::DeviceLostReason::Unknown, "injected by a test");

    let mut builder = FrameBuilder::new();
    let paint = builder.solid(opaque(255, 255, 255));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), paint));
    assert_eq!(rig.draw(&builder.finish()), FrameOutcome::Recovered);
    assert_ne!(Arc::as_ptr(rig.gpu()), dead);
    assert_eq!(rig.gpu().vulkan_extensions(), before.as_slice());
    assert_eq!(graphics.extensions(), DMA_BUF.as_slice());
    draws(&mut rig, [255, 255, 255]);
}

#[test]
fn a_renderer_asked_for_first_lands_on_a_device_that_carries_the_list() {
    let guard = device_lock();
    let graphics = graphics(&DMA_BUF);
    let rig = rig(&graphics, guard);
    let opened = graphics
        .open_gpu()
        .expect("the renderer's device is the shared one");

    assert!(Arc::ptr_eq(rig.gpu(), &opened));
    assert_eq!(enabled(&opened), DMA_BUF.as_slice());
}
