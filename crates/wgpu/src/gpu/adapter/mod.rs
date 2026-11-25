//! Finding adapters: backends, instances, ordering, and the failure that lists every rejection.

mod backends;
mod instances;

use std::sync::atomic::{AtomicU64, Ordering};

use iceblink::GpuUnavailable;

pub(crate) use self::backends::{requested_backends, tiers};
pub(crate) use self::instances::Instances;
pub use self::instances::gl_instances;

/// The environment variable that holds the device id of the adapter to prefer.
const DEVICE_ID_VARIABLE: &str = "ICEBLINK_DEVICE_ID";

static GL_ENUMERATIONS: AtomicU64 = AtomicU64::new(0);

/// Returns how many times GL adapters were enumerated in this process.
pub fn gl_enumerations() -> u64 {
    GL_ENUMERATIONS.load(Ordering::Relaxed)
}

/// Returns the device id in [`DEVICE_ID_VARIABLE`], in decimal or in hexadecimal after `0x`.
pub(crate) fn preferred_device_id() -> Option<u32> {
    let value = std::env::var(DEVICE_ID_VARIABLE).ok()?;
    let value = value.trim();
    let parsed = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => value.parse(),
    };
    parsed.ok()
}

/// Returns the key adapters are tried in: the preferred device first, then discrete, integrated,
/// virtual, software and other devices, each on the native backends before GL.
fn sort_key(info: &wgpu::AdapterInfo, preferred: Option<u32>) -> (u8, u8, u8) {
    let named = u8::from(Some(info.device) != preferred);
    let device_type = match info.device_type {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Cpu => 3,
        wgpu::DeviceType::Other => 4,
    };
    let backend = match info.backend {
        wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12 => 0,
        wgpu::Backend::Gl => 1,
        _ => 2,
    };
    (named, device_type, backend)
}

/// Reports whether the adapter draws on the processor.
pub(crate) fn is_software(info: &wgpu::AdapterInfo) -> bool {
    matches!(info.device_type, wgpu::DeviceType::Cpu)
}

/// Returns a description of the adapter for logs.
pub(crate) fn describe(info: &wgpu::AdapterInfo) -> String {
    format!(
        "{} ({:?}, {:?}, device 0x{:04x}, driver {} {})",
        info.name, info.device_type, info.backend, info.device, info.driver, info.driver_info
    )
}

/// Returns the adapters of `backends` on `instance`, in the order they are tried.
pub(crate) fn candidates(
    instance: &wgpu::Instance,
    backends: wgpu::Backends,
) -> Vec<wgpu::Adapter> {
    if backends.intersects(wgpu::Backends::GL) {
        GL_ENUMERATIONS.fetch_add(1, Ordering::Relaxed);
    }
    let preferred = preferred_device_id();
    let mut adapters = futures::executor::block_on(instance.enumerate_adapters(backends));
    adapters.sort_by_key(|adapter| sort_key(&adapter.get_info(), preferred));
    adapters
}

/// Returns the failure that lists every rejected adapter with its reason, in order.
pub(crate) fn unavailable(rejections: Vec<(String, String)>) -> GpuUnavailable {
    rejections
        .into_iter()
        .fold(GpuUnavailable::default(), |failure, (name, reason)| {
            failure.rejected(name, reason)
        })
}

#[cfg(test)]
mod tests {
    use super::{is_software, sort_key, unavailable};

    fn info(
        device: u32,
        device_type: wgpu::DeviceType,
        backend: wgpu::Backend,
    ) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "test".to_owned(),
            vendor: 0,
            device,
            device_type,
            driver: String::new(),
            driver_info: String::new(),
            backend,
            device_pci_bus_id: String::new(),
            subgroup_min_size: 32,
            subgroup_max_size: 32,
            transient_saves_memory: Some(false),
            limit_bucket: None,
        }
    }

    #[test]
    fn a_named_device_outranks_a_better_one() {
        let discrete = info(1, wgpu::DeviceType::DiscreteGpu, wgpu::Backend::Vulkan);
        let integrated = info(2, wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Vulkan);
        assert!(sort_key(&discrete, None) < sort_key(&integrated, None));
        assert!(sort_key(&integrated, Some(2)) < sort_key(&discrete, Some(2)));
    }

    #[test]
    fn only_a_processor_adapter_is_software() {
        assert!(is_software(&info(
            1,
            wgpu::DeviceType::Cpu,
            wgpu::Backend::Vulkan
        )));
        for real in [
            wgpu::DeviceType::DiscreteGpu,
            wgpu::DeviceType::IntegratedGpu,
            // A real card behind a hypervisor, which draws on that card.
            wgpu::DeviceType::VirtualGpu,
        ] {
            assert!(
                !is_software(&info(1, real, wgpu::Backend::Gl)),
                "{real:?} draws on a card"
            );
        }
    }

    #[test]
    fn the_failure_lists_every_rejection_in_order() {
        let failure = unavailable(vec![
            ("first".to_owned(), "no device".to_owned()),
            ("second".to_owned(), "surface refused".to_owned()),
        ]);
        assert_eq!(failure.candidates.len(), 2);
        assert_eq!(failure.candidates[0].name, "first");
        assert_eq!(failure.candidates[1].reason, "surface refused");
    }
}
