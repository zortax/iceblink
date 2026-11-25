//! The application shaders an output's renderer holds.

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::{ClientId, ShaderDesc, ShaderOp};

use crate::naive::scene::Scene;

/// A client's shader: its client, slot and generation.
pub(crate) type ShaderKey = (ClientId, u32, u32);

/// The shaders a renderer holds, by frame shader slot.
///
/// A shader is created on the renderer by the first frame that draws with it and destroyed by the
/// first frame after its client removed it.
#[derive(Default)]
pub(crate) struct ShaderSlots {
    held: HashMap<ShaderKey, u32>,
    free: Vec<u32>,
    next: u32,
}

impl ShaderSlots {
    /// Returns the frame slot of `key`, adding a `Create` to `ops` when the renderer does not
    /// hold the shader yet.
    pub(crate) fn slot(
        &mut self,
        key: ShaderKey,
        desc: &Arc<ShaderDesc>,
        ops: &mut Vec<ShaderOp>,
    ) -> u32 {
        if let Some(slot) = self.held.get(&key) {
            return *slot;
        }

        let slot = self.free.pop().unwrap_or_else(|| {
            self.next += 1;
            self.next - 1
        });
        self.held.insert(key, slot);
        ops.push(ShaderOp::Create {
            shader: slot,
            desc: desc.clone(),
        });
        slot
    }

    /// Adds a `Destroy` to `ops` for every held shader its client removed.
    pub(crate) fn destroy_removed(&mut self, scene: &Scene, ops: &mut Vec<ShaderOp>) {
        let free = &mut self.free;
        self.held.retain(|(client, slot, generation), shader| {
            let live = scene
                .clients
                .get(client)
                .and_then(|client| client.shaders.generation_at(*slot))
                == Some(*generation);
            if !live {
                ops.push(ShaderOp::Destroy { shader: *shader });
                free.push(*shader);
            }
            live
        });
    }

    /// Forgets every shader, as for a renderer that holds none.
    pub(crate) fn forget(&mut self) {
        *self = Self::default();
    }
}
