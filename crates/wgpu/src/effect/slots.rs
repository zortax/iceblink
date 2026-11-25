//! A renderer's shader slots, applied from the frame's shader work.

use iceblink::ShaderOp;

use crate::effect::registry::{EffectKey, Effects};
use crate::gpu::device::Gpu;

/// The shader in each frame shader slot, as a key into the device's shaders.
#[derive(Debug, Default)]
pub(crate) struct EffectSlots {
    slots: Vec<Option<EffectKey>>,
}

impl EffectSlots {
    /// Applies `ops` in order.
    ///
    /// A shader that cannot be compiled is logged and leaves its slot empty.
    pub(crate) fn apply(&mut self, gpu: &Gpu, effects: &mut Effects, ops: &[ShaderOp]) {
        for op in ops {
            match op {
                ShaderOp::Create { shader, desc } => {
                    self.clear(effects, *shader);
                    match effects.hold(gpu, desc) {
                        Ok(key) => self.set(*shader, Some(key)),
                        Err(error) => {
                            tracing::error!(shader, "an application shader was refused: {error}");
                        }
                    }
                }
                ShaderOp::Destroy { shader } => self.clear(effects, *shader),
            }
        }
    }

    /// Returns the shader in slot `shader`, or `None` for an empty slot.
    pub(crate) fn get(&self, shader: u32) -> Option<EffectKey> {
        self.slots.get(shader as usize).copied().flatten()
    }

    fn set(&mut self, shader: u32, key: Option<EffectKey>) {
        let at = shader as usize;
        if self.slots.len() <= at {
            self.slots.resize(at + 1, None);
        }
        self.slots[at] = key;
    }

    fn clear(&mut self, effects: &mut Effects, shader: u32) {
        if let Some(key) = self.get(shader) {
            effects.release(key);
        }
        self.set(shader, None);
    }
}
