//! What opening a device enumerates before it opens one.
//!
//! A hardware adapter of a native backend opens without GL being enumerated. A software adapter
//! opens only after GL offered no hardware.

use iceblink_wgpu::{Builder, gl_enumerations, gl_instances, wgpu};

// The counts are per process, so this test keeps a binary of its own.
#[test]
fn gl_is_enumerated_only_when_no_native_hardware_adapter_opens() {
    assert_eq!(gl_enumerations(), 0, "nothing has asked for an adapter yet");
    assert_eq!(gl_instances(), 0, "nothing has created an instance yet");

    let renderer = Builder::new()
        .offscreen([64, 64], wgpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    let info = renderer.gpu().adapter().get_info();

    assert_ne!(info.backend, wgpu::Backend::Gl, "a native backend opened");
    if info.device_type == wgpu::DeviceType::Cpu {
        // A software adapter is opened only after every tier, GL's included, offered no hardware.
        assert!(gl_enumerations() > 0, "GL was searched for hardware first");
    } else {
        assert_eq!(gl_enumerations(), 0, "GL's adapters were never asked for");
        assert_eq!(gl_instances(), 0, "GL's driver was never loaded");
    }
}
