//! The items a frame draws from its own lanes, and its externals and paths.

use iceblink::geom::{Point, Rect};
use iceblink::{
    ColorSprite, FRAME_NO_CLIP, FrameExternal, FrameMarkPayload, FrameMarks, FramePath,
    FrameShadow, ItemSource, Lane, Line, PassKey, PlannedPass, PlannedPath, Quad, ShaderQuad,
    Sprite,
};

use crate::conformance::draw::builder::FrameBuilder;
use crate::conformance::draw::builder::batches::Entry;
use crate::conformance::draw::builder::ink::aligned;

impl FrameBuilder {
    /// Adds a quad drawn from the frame.
    pub fn quad(&mut self, quad: Quad) {
        let ink = self.ink(quad.bounds, quad.space.0);
        push_transient(&mut self.frame.lanes.quads, quad);
        self.entries.push((Entry::Quad, ink));
    }

    /// Adds a shader quad drawn from the frame.
    pub fn shader_quad(&mut self, quad: ShaderQuad) {
        let ink = self.ink(quad.bounds, quad.space.0);
        push_transient(&mut self.frame.lanes.shader_quads, quad);
        self.entries
            .push((Entry::ShaderQuad(quad.shader, quad.params), ink));
    }

    /// Adds a shadow drawn from the frame.
    pub fn shadow(&mut self, shadow: FrameShadow) {
        let ink = self.ink(shadow.bounds, shadow.shadow.space.0);
        push_transient(&mut self.frame.lanes.shadows, shadow);
        self.entries.push((Entry::Shadow, ink));
    }

    /// Adds a line drawn from the frame.
    pub fn line(&mut self, line: Line) {
        let ink = self.ink(line.bounds, line.space.0);
        push_transient(&mut self.frame.lanes.lines, line);
        self.entries.push((Entry::Line, ink));
    }

    /// Adds a single-channel sprite drawn from the frame.
    pub fn mono(&mut self, sprite: Sprite) {
        let ink = self.ink(sprite.bounds, sprite.space);
        push_transient(&mut self.frame.lanes.mono, sprite);
        self.entries.push((Entry::Mono(sprite.tile.page), ink));
    }

    /// Adds a per-channel sprite drawn from the frame.
    pub fn subpixel(&mut self, sprite: Sprite) {
        let ink = self.ink(sprite.bounds, sprite.space);
        push_transient(&mut self.frame.lanes.subpixel, sprite);
        self.entries.push((Entry::Subpixel(sprite.tile.page), ink));
    }

    /// Adds a colour sprite drawn from the frame.
    pub fn color(&mut self, sprite: ColorSprite) {
        let ink = self.ink(sprite.frame, sprite.space);
        push_transient(&mut self.frame.lanes.color, sprite);
        self.entries.push((Entry::Color(sprite.tile.page), ink));
    }

    /// Adds a marks item drawn from the frame with `payload`.
    pub fn marks(&mut self, mut marks: FrameMarks, payload: FrameMarkPayload) {
        let ink = self.ink(marks.marks.bounds, marks.marks.space.0);
        marks.marks.payload = self.frame.lanes.mark_payloads.len() as u32;
        self.frame.lanes.mark_payloads.push(payload);
        push_transient(&mut self.frame.lanes.marks, marks);
        self.entries.push((Entry::Marks, ink));
    }

    /// Adds an external quad.
    pub fn external(&mut self, external: FrameExternal) {
        let index = self.frame.externals.len() as u32;
        let ink = self.ink(external.quad.bounds, external.quad.space.0);
        self.frame.externals.push(external);
        self.entries.push((Entry::External(index), ink));
    }

    /// Adds a path in a vector pass of its own, whose composite binds the frame clip slot `clip`.
    pub fn path(&mut self, path: FramePath, clip: u32) {
        let index = self.frame.paths.len() as u32;
        let ink = path.ink;
        let region = aligned(ink, self.output);
        self.frame.paths.push(path);

        let item = self.frame.vector_plan.items.len() as u32;
        self.frame.vector_plan.items.push(PlannedPath {
            path: index,
            residual: FRAME_NO_CLIP,
            clip,
            ink: Rect::new(
                Point::new(
                    ink.origin.x - region.origin.x,
                    ink.origin.y - region.origin.y,
                ),
                ink.size,
            ),
        });

        let pass = self.frame.vector_plan.passes.len() as u32;
        self.frame.vector_plan.passes.push(PlannedPass {
            items: item..item + 1,
            region,
            clip,
            instanced: false,
            clamped: false,
            overhangs: false,
            key: PassKey::NONE,
        });
        self.entries.push((Entry::Path(pass), ink));
    }
}

/// Appends `item` to `lane` as a transient instance drawn after the lane's others.
fn push_transient<T>(lane: &mut Lane<T>, item: T) {
    lane.sources
        .push(ItemSource::transient(lane.items.len() as u32));
    lane.items.push(item);
}
