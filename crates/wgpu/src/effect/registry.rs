//! Application shaders compiled by content, and the pipelines built from them.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use iceblink::{ShaderDesc, ShaderMode};
use rustc_hash::FxHasher;

use crate::effect::compile::{compile, draws_rectangle};
use crate::gpu::device::Gpu;
use crate::gpu::pipeline_cache::PipelineCache;
use crate::pipeline::layout::Layouts;

/// Identifies a compiled shader by the content of its description.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct EffectKey(u64);

impl EffectKey {
    pub(crate) fn of(desc: &ShaderDesc) -> Self {
        let mut hasher = FxHasher::default();
        desc.mode.hash(&mut hasher);
        desc.source.wgsl.hash(&mut hasher);
        desc.params_size.hash(&mut hasher);
        if let Some(prebuilt) = &desc.source.prebuilt {
            prebuilt.format.hash(&mut hasher);
            prebuilt.bytes.hash(&mut hasher);
        }
        Self(hasher.finish())
    }
}

/// A compiled shader, and how many holds it has.
#[derive(Debug)]
struct Registered {
    mode: ShaderMode,
    module: wgpu::ShaderModule,
    holders: u32,
}

/// Compiled shaders and the pipelines built from them, per target format.
#[derive(Debug, Default)]
pub(crate) struct Effects {
    registered: HashMap<EffectKey, Registered>,
    built: HashMap<(EffectKey, wgpu::TextureFormat), wgpu::RenderPipeline>,
    /// The shaders and reasons a dropped draw was logged for.
    reported: std::cell::RefCell<std::collections::HashSet<(EffectKey, &'static str)>>,
}

impl Effects {
    /// Takes a hold on the shader of `desc`.
    ///
    /// The shader is compiled on its first hold.
    ///
    /// Fails when the description does not pass [`check_shader`](crate::effect::check_shader).
    pub(crate) fn hold(&mut self, gpu: &Gpu, desc: &ShaderDesc) -> Result<EffectKey, String> {
        let key = EffectKey::of(desc);
        if let Some(registered) = self.registered.get_mut(&key) {
            registered.holders += 1;
            return Ok(key);
        }
        let module = compile(gpu, desc, "iceblink.shader.application")?;
        self.registered.insert(
            key,
            Registered {
                mode: desc.mode,
                module,
                holders: 1,
            },
        );
        Ok(key)
    }

    /// Drops a hold, and the shader with its pipelines once nothing holds it.
    pub(crate) fn release(&mut self, key: EffectKey) {
        let Some(registered) = self.registered.get_mut(&key) else {
            return;
        };
        registered.holders = registered.holders.saturating_sub(1);
        if registered.holders == 0 {
            self.registered.remove(&key);
            self.built.retain(|(held, _), _| *held != key);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.registered.len()
    }

    /// Logs once per shader and reason that a draw of the shader was dropped.
    pub(crate) fn note_undrawable(&self, key: EffectKey, why: &'static str) {
        if self.reported.borrow_mut().insert((key, why)) {
            tracing::warn!(
                registered = self.registered.contains_key(&key),
                "an application shader could not be drawn: {why}"
            );
        }
    }

    /// Returns the pipeline of `key` for targets of `format`, building it on first use.
    pub(crate) fn pipeline(
        &mut self,
        gpu: &Gpu,
        layouts: &Layouts,
        cache: &PipelineCache,
        key: EffectKey,
        format: wgpu::TextureFormat,
    ) -> Option<&wgpu::RenderPipeline> {
        let effect = self.registered.get(&key)?;
        Some(
            self.built
                .entry((key, format))
                .or_insert_with(|| build(gpu, layouts, cache, effect, format)),
        )
    }
}

/// Builds the pipeline of `effect` for targets of `format`, through `cache`.
fn build(
    gpu: &Gpu,
    layouts: &Layouts,
    cache: &PipelineCache,
    effect: &Registered,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let rectangle = draws_rectangle(effect.mode);
    let groups: &[Option<&wgpu::BindGroupLayout>] = if rectangle {
        &[
            Some(&layouts.frame),
            Some(&layouts.instances),
            Some(&layouts.effect),
        ]
    } else {
        &[Some(&layouts.filtered), Some(&layouts.effect)]
    };
    let label = "iceblink.pipeline.application";
    let pipeline_layout = gpu
        .device()
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: groups,
            immediate_size: 0,
        });
    let entry = |name: Option<&'static str>| name.unwrap_or("main");
    gpu.device()
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &effect.module,
                entry_point: Some(entry(iceblink_wgsl::vertex_entry(effect.mode))),
                compilation_options: Default::default(),
                buffers: if rectangle {
                    crate::pipeline::ORDER_LAYOUT
                } else {
                    &[]
                },
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &effect.module,
                entry_point: Some(entry(iceblink_wgsl::fragment_entry(effect.mode))),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: rectangle.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: cache.handle(),
        })
}
