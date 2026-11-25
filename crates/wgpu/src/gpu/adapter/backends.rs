//! The backends adapters are looked for on.

/// The environment variable that holds the backends to look for adapters on.
const BACKENDS_VARIABLE: &str = "ICEBLINK_BACKENDS";

/// Returns the backends of the platform.
fn default_backends() -> wgpu::Backends {
    #[cfg(target_vendor = "apple")]
    {
        wgpu::Backends::METAL
    }
    #[cfg(target_os = "windows")]
    {
        wgpu::Backends::DX12 | wgpu::Backends::GL
    }
    #[cfg(target_arch = "wasm32")]
    {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "windows", target_arch = "wasm32")))]
    {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    }
}

/// Returns `backends` split into the native backends and then GL, leaving out an empty tier.
pub(crate) fn tiers(backends: wgpu::Backends) -> Vec<wgpu::Backends> {
    let primary = backends - wgpu::Backends::GL;
    let fallback = backends & wgpu::Backends::GL;
    [primary, fallback]
        .into_iter()
        .filter(|tier| !tier.is_empty())
        .collect()
}

/// Returns the backends in [`BACKENDS_VARIABLE`], or the platform's backends when it is not set.
pub(crate) fn requested_backends() -> wgpu::Backends {
    match std::env::var(BACKENDS_VARIABLE) {
        Err(_) => default_backends(),
        Ok(value) => parse_backends(&value),
    }
}

/// Returns the backends a comma-separated list names, ignoring unknown names.
fn parse_backends(value: &str) -> wgpu::Backends {
    let mut backends = wgpu::Backends::empty();
    for word in value.split(',') {
        match word.trim().to_ascii_lowercase().as_str() {
            "vulkan" => backends |= wgpu::Backends::VULKAN,
            "metal" => backends |= wgpu::Backends::METAL,
            "dx12" | "d3d12" => backends |= wgpu::Backends::DX12,
            "gl" | "gles" | "opengl" | "webgl" | "webgl2" => backends |= wgpu::Backends::GL,
            "webgpu" => backends |= wgpu::Backends::BROWSER_WEBGPU,
            "all" => backends |= default_backends(),
            _ => {}
        }
    }
    backends
}

#[cfg(test)]
mod tests {
    use super::{parse_backends, tiers};

    #[test]
    fn gl_is_a_tier_of_its_own_behind_the_native_backends() {
        assert_eq!(
            tiers(wgpu::Backends::VULKAN | wgpu::Backends::GL),
            vec![wgpu::Backends::VULKAN, wgpu::Backends::GL],
            "the fallback is a second tier, not part of the first"
        );
        assert_eq!(
            tiers(wgpu::Backends::METAL | wgpu::Backends::GL),
            vec![wgpu::Backends::METAL, wgpu::Backends::GL],
            "every native backend outranks the fallback, whatever the platform"
        );
    }

    #[test]
    fn an_unrecognised_backend_list_asks_for_nothing() {
        assert_eq!(parse_backends("vulkan"), wgpu::Backends::VULKAN);
        assert_eq!(parse_backends("metal"), wgpu::Backends::METAL);
        assert_eq!(parse_backends("dx12"), wgpu::Backends::DX12);
        assert_eq!(
            parse_backends("gl, vulkan"),
            wgpu::Backends::VULKAN | wgpu::Backends::GL
        );
        assert!(parse_backends("none").is_empty());
        assert!(parse_backends("nonsense").is_empty());
    }
}
