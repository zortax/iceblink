//! The instances adapters are enumerated from, one per backend tier.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::gpu::adapter::tiers;

static GL_INSTANCES: AtomicU64 = AtomicU64::new(0);

/// Returns how many GL instances were created in this process.
pub fn gl_instances() -> u64 {
    GL_INSTANCES.load(Ordering::Relaxed)
}

/// One instance per backend tier, each created on first use.
#[derive(Clone, Debug)]
pub(crate) struct Instances {
    tiers: Vec<(wgpu::Backends, OnceLock<wgpu::Instance>)>,
}

impl Instances {
    /// Returns the instances of the tiers of `backends`, or of `backends` as one tier when it has
    /// no tiers.
    pub(crate) fn new(backends: wgpu::Backends) -> Self {
        let mut tiers = tiers(backends);
        if tiers.is_empty() {
            tiers.push(backends);
        }
        Self {
            tiers: tiers
                .into_iter()
                .map(|tier| (tier, OnceLock::new()))
                .collect(),
        }
    }

    /// Returns one tier of `backends`, with `instance` as its instance.
    pub(crate) fn existing(instance: wgpu::Instance, backends: wgpu::Backends) -> Self {
        Self {
            tiers: vec![(backends, OnceLock::from(instance))],
        }
    }

    /// Returns the instance of the first tier.
    pub(crate) fn primary(&self) -> &wgpu::Instance {
        let (tier, instance) = &self.tiers[0];
        instance.get_or_init(|| instance_of(*tier))
    }

    /// Returns each tier's backends and instance, in order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (wgpu::Backends, &wgpu::Instance)> {
        self.tiers
            .iter()
            .map(|(tier, instance)| (*tier, instance.get_or_init(|| instance_of(*tier))))
    }
}

/// Returns an instance of `backends`.
///
/// A GL instance is new on every call. Every other instance is shared by the whole process and
/// created on first use.
fn instance_of(backends: wgpu::Backends) -> wgpu::Instance {
    if backends.intersects(wgpu::Backends::GL) {
        // A GL instance owns an EGL context, which threads cannot make current at the same time.
        let _opening = super::opening();
        return create(backends);
    }
    // The shared instances are never destroyed. Destroying the last Vulkan instance unloads the
    // driver, and a thread creating an instance meanwhile crashed in the NVIDIA ICD (driver
    // 610.43, loader 1.4.350).
    static SHARED: Mutex<Vec<(wgpu::Backends, wgpu::Instance)>> = Mutex::new(Vec::new());
    let mut shared = SHARED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, instance)) = shared.iter().find(|(tier, _)| *tier == backends) {
        return instance.clone();
    }
    let instance = {
        let _opening = super::opening();
        create(backends)
    };
    shared.push((backends, instance.clone()));
    instance
}

/// Creates an instance of `backends`.
fn create(backends: wgpu::Backends) -> wgpu::Instance {
    if backends.intersects(wgpu::Backends::GL) {
        GL_INSTANCES.fetch_add(1, Ordering::Relaxed);
    }
    wgpu::Instance::new(instance_descriptor(backends))
}

/// Returns the instance flags of the build and the environment, without indirect call validation
/// in a release build.
fn instance_flags() -> wgpu::InstanceFlags {
    let mut flags = wgpu::InstanceFlags::from_build_config();
    if !cfg!(debug_assertions) {
        flags.remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
    }
    flags.with_env()
}

fn instance_descriptor(backends: wgpu::Backends) -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        backends,
        flags: instance_flags(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    }
}
