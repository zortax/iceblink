//! The rasterisation of one round.

use std::ops::Range;
use std::sync::Arc;

use iceblink_wgpu::vector::{ResourcedPlan, VectorError, VectorFrame, VectorTarget};
use vello::{AaConfig, RenderParams};

use super::{VelloRaster, encode};

impl VelloRaster {
    /// Encodes and rasterises the round of `frame`, one rasterisation per layer.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Device`] when the path renderer fails, and
    /// [`VectorError::OutOfCapacity`] when a pass of the round has no target.
    pub(super) fn prepare_round(&mut self, frame: &VectorFrame<'_>) -> Result<(), VectorError> {
        if frame.is_empty() {
            return Ok(());
        }

        self.group(frame.plan, frame.passes());
        let before = self.last;
        let shared = Arc::clone(&self.shared);
        let renderer = &mut *shared.lock();
        for layer in 0..self.layered.len() {
            let [width, height] = self.encode_layer(frame, layer);
            if width == 0 || height == 0 {
                continue;
            }
            let Some(view) = self.scratch.view(layer as u32) else {
                continue;
            };
            shared
                .measuring(|| {
                    renderer.render_to_texture(
                        &self.device,
                        &self.queue,
                        &self.scenes[layer],
                        view,
                        // To the far corner of the layer's passes, since a rasterisation costs
                        // its whole area.
                        &render_params(width, height),
                    )
                })
                .map_err(|error| VectorError::Device {
                    detail: error.to_string(),
                })?;
            self.renders += 1;
        }

        let unclippable = self.last.unclippable - before.unclippable;
        if unclippable > 0 {
            tracing::warn!(
                paths = unclippable,
                "paths left undrawn because a residual clip has no outline"
            );
        }

        match frame
            .passes()
            .find(|&index| frame.plan.passes[index].target == VectorTarget::NONE)
        {
            None => Ok(()),
            Some(prepared) => Err(VectorError::OutOfCapacity {
                detail: format!("pass {prepared} has no scratch target"),
                prepared,
            }),
        }
    }

    /// Sorts the passes of `round` into the layer each one was placed in.
    // One rasterisation per layer: five hundred passes in one layer cost one rasterisation.
    fn group(&mut self, plan: &ResourcedPlan, round: Range<usize>) {
        let layers = (self.scratch.layers() as usize).max(1);
        for bucket in &mut self.layered {
            bucket.clear();
        }
        self.layered.resize_with(layers, Vec::new);
        self.scenes.resize_with(layers, vello::Scene::new);

        for index in round {
            let target = plan.passes[index].target.0;
            if let Some(bucket) = usize::try_from(target)
                .ok()
                .and_then(|layer| self.layered.get_mut(layer))
            {
                bucket.push(index);
            }
        }
    }

    /// Encodes the passes grouped into `layer` into its scene, and returns the far corner of
    /// their bins.
    ///
    /// Passes with an empty bin are skipped. A layer without passes returns zero.
    fn encode_layer(&mut self, frame: &VectorFrame<'_>, layer: usize) -> [u32; 2] {
        let scene = &mut self.scenes[layer];
        scene.reset();

        let (mut width, mut height) = (0, 0);
        for &index in &self.layered[layer] {
            let pass = &frame.plan.passes[index];
            if pass.raster_region.size.width <= 0 || pass.raster_region.size.height <= 0 {
                continue;
            }
            let encoded = encode::pass(scene, &mut self.encodings, frame, pass, &mut self.stops);
            self.last.add(encoded);
            width = width.max(pass.raster_region.right().max(0) as u32);
            height = height.max(pass.raster_region.bottom().max(0) as u32);
            self.passes += 1;
        }
        [width, height]
    }
}

/// Returns the parameters of a rasterisation `width` by `height` texels.
pub(super) fn render_params(width: u32, height: u32) -> RenderParams {
    RenderParams {
        // Unpainted texels must composite as nothing.
        base_color: peniko::Color::TRANSPARENT,
        width,
        height,
        antialiasing_method: AaConfig::Area,
    }
}
