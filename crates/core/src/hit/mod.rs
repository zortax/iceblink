//! Hit testing: the items under a device point, front to back.
//!
//! The walk visits the paint order of an output's root from last to first. Order nodes and chunks
//! whose stored ink misses the point are skipped, as are the rows of an ordered node that miss
//! it.

mod clip;
mod distance;
mod plane;
mod shape;
mod walk;

use iceblink::smallvec::SmallVec;
use iceblink::{HitResult, OutputId};

use crate::compositor::CompositorImpl;

pub(crate) use self::walk::{Hit, refs};

impl CompositorImpl {
    /// Returns every item of `output` under the device point `point`, front to back.
    pub(crate) fn hits(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        let mut out = SmallVec::new();
        self.walk_hits(output, point, &mut |hit| {
            out.push(hit.result);
            true
        });
        out
    }

    /// Returns the frontmost item of `output` under the device point `point`.
    pub(crate) fn frontmost(&self, output: OutputId, point: [f32; 2]) -> Option<Hit> {
        let mut first = None;
        self.walk_hits(output, point, &mut |hit| {
            first = Some(hit);
            false
        });
        first
    }

    /// Calls `found` with each item under `point`, front to back, until it returns false.
    fn walk_hits(&self, output: OutputId, point: [f32; 2], found: &mut dyn FnMut(Hit) -> bool) {
        let Some((client, root)) = self.outputs.get(&output).and_then(|output| output.root) else {
            return;
        };
        let Some(state) = self.clients.get(&client) else {
            return;
        };
        if !point.iter().all(|value| value.is_finite()) {
            return;
        }
        walk::Walk::new(&state.scene, &self.content.images, client, point, found).run(root.slot());
    }
}
