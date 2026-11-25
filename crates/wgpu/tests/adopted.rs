//! Drawing on a device the application opened.
//!
//! The adopted device keeps the application's features and device-lost callback, a frame lands in
//! a texture the application created, and neither graphics nor a renderer over an adopted device
//! open a device of their own after a loss.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, device_lock};
use common::present::Rig;
use iceblink::Renderer as _;
use iceblink::{Color, Frame, FrameOutcome, RenderTarget, SkipReason};
use iceblink_wgpu::{Builder, Gpu, Pixels, SharedGraphics, Supplied, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A device as an application opens it.
struct Host {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Whether the application's device-lost callback ran.
    lost: Arc<AtomicBool>,
    /// A feature the application asked for and the renderer never asks for, if the adapter has it.
    extra: wgpu::Features,
}

/// Opens a device as an application does, with a device-lost callback of its own.
fn open_host() -> Host {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = futures::executor::block_on(
        instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
    )
    .unwrap_or_else(|error| panic!("no usable graphics device: {error}"));
    let extra = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
    let (device, queue) =
        futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("test.host"),
            required_features: extra | (adapter.features() & wgpu::Features::DUAL_SOURCE_BLENDING),
            ..Default::default()
        }))
        .expect("the adapter opens a device");
    let lost = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&lost);
    device.set_device_lost_callback(move |_, _| seen.store(true, Ordering::SeqCst));
    Host {
        instance,
        adapter,
        device,
        queue,
        lost,
        extra,
    }
}

/// Returns the device of `host` as the renderer's.
fn adopt(host: &Host) -> Arc<Gpu> {
    Gpu::from_existing(
        host.instance.clone(),
        host.adapter.clone(),
        host.device.clone(),
        host.queue.clone(),
    )
}

/// Returns a frame of one green square.
fn square() -> Frame {
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(opaque(0, 200, 100));
    builder.quad(filled(rect(16.0, 16.0, 64.0, 64.0), paint));
    builder.finish()
}

/// Reports the loss of `gpu` as the application does.
fn report_loss(gpu: &Gpu) {
    gpu.loss().report(
        wgpu::DeviceLostReason::Unknown,
        "reported by the application",
    );
}

#[test]
fn an_adopted_device_keeps_the_applications_features_and_reports_its_own_capabilities() {
    let _guard = device_lock();
    let host = open_host();
    let gpu = adopt(&host);

    assert!(gpu.is_adopted());
    assert!(gpu.device().features().contains(host.extra));
    assert_eq!(
        gpu.capabilities().subpixel_text,
        host.device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING),
        "capabilities are read off the adopted device"
    );
    assert!(gpu.capabilities().max_texture_size > 0);
    assert!(gpu.vulkan_extensions().is_empty());
}

#[test]
fn a_frame_lands_premultiplied_in_the_applications_texture() {
    let guard = device_lock();
    let host = open_host();
    let graphics = SharedGraphics::with_gpu(adopt(&host));
    assert!(graphics.is_adopted());
    let gpu = graphics
        .open_gpu()
        .expect("the adopted device is the primary");
    assert!(gpu.device() == &host.device);

    let ui = host.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test.host.ui"),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let renderer = Supplied::renderer(&graphics, vec![ui.clone()])
        .expect("a texture on the adopted device is accepted");
    let mut rig = Rig::new(renderer, guard);
    rig.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: false,
    });

    let mut builder = FrameBuilder::new();
    builder.frame_mut().opaque = false;
    let alpha = 128.0 / 255.0;
    let red = builder.solid(Color([alpha, 0.0, 0.0, alpha]));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32 / 2.0, SIDE as f32), red));
    assert!(rig.draw(&builder.finish()).stats().is_some());

    let pixels = Pixels::of_texture(&gpu, &ui);
    let inside = pixels.rgba(SIDE as i32 / 4, SIDE as i32 / 2);
    let near = |a: u8, b: u8| a.abs_diff(b) <= 1;
    assert!(
        near(inside[0], 128) && inside[1] == 0 && inside[2] == 0 && near(inside[3], 128),
        "half-transparent red is premultiplied: {inside:?}"
    );
    assert_eq!(
        pixels.rgba(SIDE as i32 * 3 / 4, SIDE as i32 / 2),
        [0, 0, 0, 0],
        "undrawn pixels stay transparent"
    );
}

#[test]
fn the_applications_device_lost_callback_is_not_replaced() {
    let _guard = device_lock();
    let host = open_host();
    let gpu = adopt(&host);

    host.device.destroy();
    let _ = host.device.poll(wgpu::PollType::wait_indefinitely());

    assert!(
        host.lost.load(Ordering::SeqCst),
        "the application's callback ran"
    );
    assert!(
        !gpu.loss().is_lost(),
        "nothing reports the loss until the application does"
    );

    (gpu.loss().observer())(
        wgpu::DeviceLostReason::Destroyed,
        "reported by the application",
    );

    assert!(gpu.loss().is_lost());
}

#[test]
fn adopted_graphics_never_open_a_device_of_their_own() {
    let _guard = device_lock();
    let host = open_host();
    let gpu = adopt(&host);
    let graphics = SharedGraphics::with_gpu(Arc::clone(&gpu));

    report_loss(&gpu);

    assert!(
        graphics.open_gpu().is_err(),
        "a lost adopted device is not replaced by one opened here"
    );
    assert!(graphics.renderer_offscreen([SIDE, SIDE], FORMAT).is_err());

    let recovered = open_host();
    let replacement = adopt(&recovered);
    graphics.adopt(Arc::clone(&replacement));
    let reopened = graphics
        .open_gpu()
        .expect("the adopted replacement is usable");
    assert!(Arc::ptr_eq(&reopened, &replacement));
}

#[test]
fn adopting_the_primary_again_keeps_it() {
    let _guard = device_lock();
    let host = open_host();
    let gpu = adopt(&host);
    let graphics = SharedGraphics::with_gpu(Arc::clone(&gpu));

    graphics.adopt(Arc::clone(&gpu));

    let current = graphics.gpu().expect("the primary is set at construction");
    assert!(Arc::ptr_eq(&current, &gpu));
}

#[test]
fn a_renderer_on_the_applications_device_waits_for_a_replacement_after_a_loss() {
    let guard = device_lock();
    let host = open_host();
    let gpu = adopt(&host);
    let offered: Rc<RefCell<Option<Arc<Gpu>>>> = Rc::default();
    let asked = Rc::clone(&offered);
    let renderer = Builder::from_gpu(Arc::clone(&gpu))
        .with_replacement_gpu(Box::new(move |_lost| asked.borrow_mut().take()))
        .offscreen([SIDE, SIDE], FORMAT)
        .expect("the adopted device presents offscreen");
    let mut rig = Rig::new(renderer, guard);
    let before = rig.present(&square());

    report_loss(&gpu);
    let unavailable = FrameOutcome::Skipped(SkipReason::DeviceUnavailable);
    assert_eq!(rig.draw(&square()), unavailable);
    assert_eq!(rig.draw(&square()), unavailable, "no replacement yet");
    assert!(Arc::ptr_eq(rig.gpu(), &gpu));

    let recovered = open_host();
    let replacement = adopt(&recovered);
    *offered.borrow_mut() = Some(Arc::clone(&replacement));
    assert_eq!(rig.draw(&square()), FrameOutcome::Recovered);
    assert!(Arc::ptr_eq(rig.gpu(), &replacement));
    let after = rig.present(&square());
    assert_eq!(before.max_difference(&after), 0);
}

#[test]
fn a_renderer_on_the_applications_device_without_a_replacement_stays_unavailable() {
    let guard = device_lock();
    let host = open_host();
    let gpu = adopt(&host);
    let renderer = Builder::from_gpu(Arc::clone(&gpu))
        .offscreen([SIDE, SIDE], FORMAT)
        .expect("the adopted device presents offscreen");
    let mut rig = Rig::new(renderer, guard);
    rig.present(&square());

    report_loss(&gpu);
    assert_eq!(
        rig.draw(&square()),
        FrameOutcome::Skipped(SkipReason::DeviceUnavailable)
    );
    assert!(
        Arc::ptr_eq(rig.gpu(), &gpu),
        "no device of its own was opened"
    );
}
