//! Frame slots of paints, parameter blocks and shaders, and filter chains that use them.

use std::sync::Arc;

use iceblink::{Filter, FrameFilter, PaintKind, PaintRef, ShaderReads};

use crate::naive::frame::NO_PARAMS;
use crate::naive::frame::builder::Builder;
use crate::naive::frame::paint;
use crate::naive::frame::slots::PaintKey;

impl Builder<'_> {
    /// Returns the frame reference of a paint reference.
    ///
    /// Inline colours become solid references to a frame slot holding the colour.
    pub(crate) fn paint(&mut self, paint: PaintRef) -> PaintRef {
        if paint.kind == PaintKind::NONE {
            return PaintRef::NONE;
        }

        let (key, kind) = match paint.inline_color() {
            Some(_) => (PaintKey::Inline(paint.index), PaintKind::SOLID),
            None => (PaintKey::Slot(self.env.client_id, paint.index), paint.kind),
        };

        let (frame_slot, first) = self.slots.paints.slot(&key);
        if first {
            let value = match paint.inline_color() {
                Some(rgba) => paint::inline(rgba),
                None => self
                    .env
                    .client
                    .paints
                    .at(paint.index)
                    .map(paint::lowered)
                    .unwrap_or_default(),
            };
            self.paints.set(frame_slot, value);
        }

        PaintRef {
            kind,
            _pad: [0; 3],
            index: frame_slot,
        }
    }

    /// Returns the frame slot of the client's parameter block at `slot`.
    pub(crate) fn params(&mut self, slot: u32) -> u32 {
        let (frame_slot, first) = self.slots.params.slot(&(self.env.client_id, slot));
        if first {
            let block = self
                .env
                .client
                .params
                .at(slot)
                .copied()
                .unwrap_or(NO_PARAMS);
            self.params.set(frame_slot, block);
        }
        frame_slot
    }

    /// Returns the frame slot of the client's shader at `slot`, creating it on the renderer when
    /// it does not hold it.
    pub(crate) fn shader(&mut self, slot: u32) -> Option<u32> {
        let client = self.env.client;
        let desc = client.shaders.at(slot)?;
        let generation = client.shaders.generation_at(slot)?;
        if desc.reads.contains(ShaderReads::TIME) {
            self.reads_time = true;
        }

        let key = (self.env.client_id, slot, generation);
        Some(
            self.shaders
                .slot(key, desc, &mut self.frame.resources.shaders),
        )
    }

    /// Returns a filter chain with its shaders and parameters as frame slots.
    ///
    /// A shader filter is left out when the renderer runs no application shaders.
    pub(crate) fn filters(&mut self, chain: &[Filter]) -> Arc<[FrameFilter]> {
        let mut lowered = Vec::with_capacity(chain.len());
        for filter in chain {
            lowered.push(match *filter {
                Filter::Blur(deviation) => FrameFilter::Blur(deviation),
                Filter::DropShadow {
                    offset,
                    std_dev,
                    color,
                } => FrameFilter::DropShadow {
                    offset,
                    std_dev,
                    color: color.quantized(),
                },
                Filter::ColorMatrix(matrix) => FrameFilter::ColorMatrix(matrix),
                Filter::Opacity(amount) => FrameFilter::Opacity(amount),
                Filter::Shader {
                    shader,
                    params,
                    reach,
                } => {
                    if !self.env.caps.custom_shaders {
                        continue;
                    }
                    let Some(shader) = self.shader(shader.slot()) else {
                        continue;
                    };
                    FrameFilter::Shader {
                        shader,
                        params: self.params(params.slot()),
                        reach,
                    }
                }
            });
        }
        lowered.into()
    }
}
