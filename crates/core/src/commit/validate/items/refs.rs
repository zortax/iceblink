//! Checking the slots an item refers to, and collecting the objects behind them.

use iceblink::{ClipRef, Color, PaintKind, PaintRef, RejectReason, ShaderMode, SpaceRef};

use super::Check;
use crate::commit::finite;
use crate::scene::kind::{Kind, ObjectRef};

impl Check<'_> {
    pub(super) fn paint(&mut self, paint: PaintRef) {
        match paint.kind {
            PaintKind::NONE => {}
            PaintKind::INLINE => {
                let [r, g, b, a] = paint.index.to_le_bytes();
                self.valid(r <= a && g <= a && b <= a);
            }
            kind if kind.is_valid() => {
                self.deps.push(ObjectRef::new(Kind::Paint, paint.index));
                match self.scene.paints.at(paint.index) {
                    None => self.valid(false),
                    Some(record) => self
                        .worst
                        .require(record.paint.kind() == kind, RejectReason::PaintKindChanged),
                }
            }
            _ => self.valid(false),
        }
    }

    pub(super) fn shader(&mut self, slot: u32, modes: &[ShaderMode]) {
        self.deps.push(ObjectRef::new(Kind::Shader, slot));
        match self.scene.shaders.at(slot) {
            None => self.valid(false),
            Some(record) => self.worst.require(
                modes.contains(&record.desc.mode),
                RejectReason::InvalidShader,
            ),
        }
    }

    pub(super) fn slot(&mut self, kind: Kind, slot: u32) {
        let object = ObjectRef::new(kind, slot);
        self.deps.push(object);
        self.valid(self.scene.is_live(object));
    }

    pub(super) fn clip(&mut self, clip: ClipRef) {
        if clip == ClipRef::INHERIT || clip == ClipRef::NONE {
            return;
        }
        match clip.local_index() {
            Some(index) => self.valid((index as usize) < self.chunk.local_clips.len()),
            None => self.slot(Kind::Clip, clip.0),
        }
    }

    pub(super) fn space(&mut self, space: SpaceRef) {
        if space != SpaceRef::INHERIT {
            self.slot(Kind::Transform, space.0);
        }
    }

    pub(super) fn color(&mut self, color: &Color) {
        self.valid(finite::color(color));
    }
}
