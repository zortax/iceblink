//! The device harness on the requested driver.

use iceblink_testing::gpu::{Driver, TestDevice, describe, read_rgba};

#[test]
fn the_device_is_on_the_requested_driver() {
    let Some(device) = TestDevice::open() else {
        return;
    };
    let info = device.info();
    assert!(
        Driver::requested().accepts(&info),
        "{:?} was requested and {} was opened",
        Driver::requested(),
        describe(&info)
    );
    eprintln!("device: {}", describe(&info));
}

#[test]
fn a_cleared_texture_reads_back_as_rgba() {
    let Some(device) = TestDevice::open() else {
        return;
    };
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
    ] {
        let texture = device.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.test-cleared"),
            size: wgpu::Extent3d {
                width: 70,
                height: 3,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("iceblink.test-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        }));
        device.queue.submit([encoder.finish()]);

        let image = read_rgba(&device.device, &device.queue, &texture);
        assert_eq!((image.width(), image.height()), (70, 3));
        assert_eq!(image.pixel(69, 2), [255, 0, 0, 255], "{format:?}");
    }
}
