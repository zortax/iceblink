//! The frame's vector passes, rasterised through `vello_cpu`, and the rasters kept by pass key.

mod brush;
mod outline;
mod raster;

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::{PassKey, VectorPlan};
use vello_cpu::{RenderContext, Resources};

pub(crate) use self::raster::{Inputs, Raster};

/// The path rasteriser and the rasters it keeps.
#[derive(Debug)]
pub(crate) struct Vectors {
    context: RenderContext,
    resources: Resources,
    kept: HashMap<(PassKey, [i32; 2]), Arc<Raster>>,
    /// Whether rasters of keyed passes are kept for the next frame.
    pub(crate) keeping: bool,
    rasterised: u64,
    reused: u64,
}

impl Default for Vectors {
    fn default() -> Self {
        Self {
            context: RenderContext::new(1, 1),
            resources: Resources::new(),
            kept: HashMap::new(),
            keeping: true,
            rasterised: 0,
            reused: 0,
        }
    }
}

impl Vectors {
    /// Returns a raster for every pass of `plan`, in plan order.
    ///
    /// A keyed pass whose raster is kept for its key and region size is composited from that
    /// raster without rasterising it. Rasters of keyed passes are kept until a frame draws no
    /// pass of their key, while keeping is on.
    pub(crate) fn rasterise(&mut self, plan: &VectorPlan, inputs: &Inputs<'_>) -> Vec<Arc<Raster>> {
        let mut drawn = Vec::with_capacity(plan.passes.len());
        let mut kept = HashMap::new();
        for pass in &plan.passes {
            let size = [pass.region.width(), pass.region.height()];
            let keyed = self.keeping && pass.key != PassKey::NONE;
            let held = keyed
                .then(|| self.kept.get(&(pass.key, size)).cloned())
                .flatten();
            let raster = match held {
                Some(raster) => {
                    self.reused += 1;
                    // The same pixels, at this pass's position.
                    Arc::new(raster.moved_to(pass.region.origin))
                }
                None => {
                    self.rasterised += 1;
                    let items = plan.items_of(pass);
                    Arc::new(raster::rasterise(
                        &mut self.context,
                        &mut self.resources,
                        inputs,
                        pass.region,
                        items,
                    ))
                }
            };
            if keyed {
                kept.insert((pass.key, size), Arc::clone(&raster));
            }
            drawn.push(raster);
        }
        self.kept = kept;
        drawn
    }

    /// Drops every kept raster, and returns the bytes they held.
    pub(crate) fn release(&mut self) -> u64 {
        let bytes = self.bytes();
        self.kept.clear();
        bytes
    }

    /// Returns the bytes the kept rasters hold.
    pub(crate) fn bytes(&self) -> u64 {
        self.kept.values().map(|raster| raster.bytes()).sum()
    }

    /// Calls `out` with how many passes were rasterised and how many reused a kept raster.
    pub(crate) fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        out("vector_passes_rasterised", self.rasterised);
        out("vector_passes_kept", self.reused);
    }
}
