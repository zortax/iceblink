//! The render pipelines of a device, built on first use for each target format.

mod background;
pub(crate) mod blur;
pub(crate) mod composite;
mod create;
pub(crate) mod effect_filter;
pub(crate) mod external;
pub(crate) mod kind;
pub(crate) mod layout;
pub(crate) mod marks;
pub(crate) mod vector;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::effect::{EffectKey, Effects};
use crate::gpu::device::Gpu;
use crate::gpu::pipeline_cache::{self, PipelineCache};
use crate::pipeline::background::Background;
use crate::pipeline::create::{build, shader_module};
use crate::pipeline::kind::PipelineKind;
use crate::pipeline::layout::Layouts;
use crate::shader::{self, Module};

pub(crate) use self::create::ORDER_LAYOUT;

/// The pipelines, shader modules, layouts and pipeline cache of one device.
#[derive(Debug)]
pub(crate) struct Pipelines {
    layouts: Layouts,
    modules: BTreeMap<Module, wgpu::ShaderModule>,
    /// The pipelines built for each target format.
    built: Vec<(wgpu::TextureFormat, Box<KindTable>)>,
    cache: PipelineCache,
    dual_source_blending: bool,
    effects: Effects,
    /// The pipelines a thread builds when the cache did not start from a file the driver
    /// accepted.
    background: Option<Background>,
}

impl Pipelines {
    /// Returns the pipelines of `gpu`, with the pipeline cache loaded from `cache_directory`, or
    /// from the default directory.
    ///
    /// # Panics
    ///
    /// In a debug build, panics when [`check_shader_layouts`](shader::check_shader_layouts) fails.
    pub(crate) fn new(gpu: &Gpu, cache_directory: Option<&Path>) -> Self {
        if cfg!(debug_assertions)
            && let Err(error) = shader::check_shader_layouts()
        {
            panic!("{error}");
        }
        Self {
            layouts: Layouts::new(gpu),
            modules: BTreeMap::new(),
            built: Vec::new(),
            cache: pipeline_cache::load(gpu, cache_directory),
            dual_source_blending: gpu.capabilities().subpixel_text,
            effects: Effects::default(),
            background: None,
        }
    }

    /// Builds each kind and target format in `wanted` that the device can draw.
    ///
    /// When they are built depends on the pipeline cache and the platform:
    ///
    /// - If the cache started from a file the driver accepted, or the platform cannot run a
    ///   thread, every pipeline is built and the cache written before the call returns.
    /// - Otherwise a thread builds them, and [`get`](Self::get) builds any it needs before the
    ///   thread does. If the thread cannot be started, each pipeline is built when first drawn
    ///   with.
    pub(crate) fn prepare(
        &mut self,
        gpu: &Arc<Gpu>,
        wanted: &[(PipelineKind, wgpu::TextureFormat)],
    ) {
        let wanted: Vec<_> = wanted
            .iter()
            .copied()
            .filter(|&(kind, format)| self.buildable(kind, format))
            .collect();
        if self.cache.loaded() || !background::available() {
            for (kind, format) in wanted {
                let _ = self.get(gpu, kind, format);
            }
            self.persist();
            return;
        }
        let cache = self.cache.handle().cloned();
        self.background = Some(Background::spawn(
            Arc::clone(gpu),
            self.layouts.clone(),
            cache,
            wanted,
        ));
    }

    pub(crate) fn effects(&self) -> &Effects {
        &self.effects
    }

    pub(crate) fn effects_mut(&mut self) -> &mut Effects {
        &mut self.effects
    }

    /// Returns the pipeline of an application shader for targets of `format`.
    pub(crate) fn effect(
        &mut self,
        gpu: &Gpu,
        key: EffectKey,
        format: wgpu::TextureFormat,
    ) -> Option<&wgpu::RenderPipeline> {
        self.effects
            .pipeline(gpu, &self.layouts, &self.cache, key, format)
    }

    pub(crate) fn layouts(&self) -> &Layouts {
        &self.layouts
    }

    /// Returns the pipeline of `kind` for targets of `format`, building it on first use.
    ///
    /// Returns `None` for a kind the device or the format cannot draw.
    pub(crate) fn get(
        &mut self,
        gpu: &Gpu,
        kind: PipelineKind,
        format: wgpu::TextureFormat,
    ) -> Option<&wgpu::RenderPipeline> {
        if !self.buildable(kind, format) {
            return None;
        }
        self.take_background();
        let column = self.column(format);
        let index = kind as usize;
        if self.built[column].1[index].is_none() {
            let source = kind.module();
            let module = self
                .modules
                .entry(source)
                .or_insert_with(|| shader_module(gpu, source));
            let pipeline = build(
                gpu,
                &self.layouts,
                self.cache.handle(),
                kind,
                format,
                module,
            );
            self.built[column].1[index] = Some(pipeline);
        }
        self.built[column].1[index].as_ref()
    }

    /// Reports whether the device can draw `kind` into targets of `format`.
    fn buildable(&self, kind: PipelineKind, format: wgpu::TextureFormat) -> bool {
        kind.suits(format) && (self.dual_source_blending || !kind.needs_dual_source_blending())
    }

    /// Returns the index of the table of pipelines for targets of `format`, adding one if needed.
    fn column(&mut self, format: wgpu::TextureFormat) -> usize {
        if let Some(column) = self.built.iter().position(|(held, _)| *held == format) {
            return column;
        }
        self.built
            .push((format, Box::new(core::array::from_fn(|_| None))));
        self.built.len() - 1
    }

    /// Moves the pipelines the thread built since the last call into the tables, and keeps any
    /// built here meanwhile.
    fn take_background(&mut self) {
        let Some(background) = &self.background else {
            return;
        };
        let (finished, built) = background.take();
        if !built.is_empty() {
            // The thread built into the cache, which is then unsaved.
            let _ = self.cache.handle();
        }
        for (kind, format, pipeline) in built {
            let column = self.column(format);
            self.built[column].1[kind as usize].get_or_insert(pipeline);
        }
        if finished {
            self.background = None;
        }
    }

    /// Writes the pipeline cache if a pipeline was built since the last write.
    pub(crate) fn persist(&self) {
        self.cache.store();
    }
}

/// The pipelines of one target format, indexed by kind.
type KindTable = [Option<wgpu::RenderPipeline>; PipelineKind::ALL.len()];
