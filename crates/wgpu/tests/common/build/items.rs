//! Instances, groups, backdrops, externals and paths drawn from the frame.

use iceblink::geom::{Point, Rect};
use iceblink::{
    ColorSprite, DeviceRect, FRAME_NO_CLIP, FrameBackdrop, FrameExternal, FrameGroup,
    FrameMarkPayload, FrameMarks, FramePath, FrameShadow, ItemSource, Line, Matrix4, PassKey,
    PlannedPass, PlannedPath, Quad, ShaderQuad, Sprite,
};

use super::FrameBuilder;
use super::batches::Entry;

impl FrameBuilder {
    /// Returns the device rectangle `bounds` covers under space `space`, with two pixels of
    /// slack and cut to the output.
    ///
    /// A space past the table counts as the identity. Bounds with a corner that maps to no finite
    /// point cover the whole output.
    pub(super) fn ink(&self, bounds: [f32; 4], space: u32) -> DeviceRect {
        let matrix = self
            .spaces
            .get(space as usize)
            .copied()
            .unwrap_or(Matrix4::IDENTITY);
        let [x, y, w, h] = bounds;
        let corners = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)];
        let mut low = (f32::INFINITY, f32::INFINITY);
        let mut high = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for (cx, cy) in corners {
            let [px, py, _] = matrix.transform_point(cx, cy, 0.0);
            low = (low.0.min(px), low.1.min(py));
            high = (high.0.max(px), high.1.max(py));
        }
        if !(low.0.is_finite() && low.1.is_finite() && high.0.is_finite() && high.1.is_finite()) {
            return self.output;
        }

        let ink = Rect::from_corners(
            Point::new(low.0.floor() as i32 - 2, low.1.floor() as i32 - 2),
            Point::new(high.0.ceil() as i32 + 2, high.1.ceil() as i32 + 2),
        );
        ink.intersection(self.output).unwrap_or(Rect::ZERO)
    }

    /// Adds a quad drawn from the frame.
    pub(crate) fn quad(&mut self, quad: Quad) {
        let ink = self.ink(quad.bounds, quad.space.0);
        let lane = &mut self.frame.lanes.quads;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(quad);
        self.entries.push((Entry::Quad, ink));
    }

    /// Adds a shader quad drawn from the frame.
    pub(crate) fn shader_quad(&mut self, quad: ShaderQuad) {
        let ink = self.ink(quad.bounds, quad.space.0);
        let lane = &mut self.frame.lanes.shader_quads;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(quad);
        self.entries
            .push((Entry::ShaderQuad(quad.shader, quad.params), ink));
    }

    /// Adds a shadow drawn from the frame.
    pub(crate) fn shadow(&mut self, shadow: FrameShadow) {
        let ink = self.ink(shadow.bounds, shadow.shadow.space.0);
        let lane = &mut self.frame.lanes.shadows;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(shadow);
        self.entries.push((Entry::Shadow, ink));
    }

    /// Adds a line drawn from the frame.
    pub(crate) fn line(&mut self, line: Line) {
        let ink = self.ink(line.bounds, line.space.0);
        let lane = &mut self.frame.lanes.lines;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(line);
        self.entries.push((Entry::Line, ink));
    }

    /// Adds a single-channel sprite drawn from the frame.
    pub(crate) fn mono(&mut self, sprite: Sprite) {
        let ink = self.ink(sprite.bounds, sprite.space);
        let lane = &mut self.frame.lanes.mono;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(sprite);
        self.entries.push((Entry::Mono(sprite.tile.page), ink));
    }

    /// Adds a per-channel sprite drawn from the frame.
    pub(crate) fn subpixel(&mut self, sprite: Sprite) {
        let ink = self.ink(sprite.bounds, sprite.space);
        let lane = &mut self.frame.lanes.subpixel;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(sprite);
        self.entries.push((Entry::Subpixel(sprite.tile.page), ink));
    }

    /// Adds a colour sprite drawn from the frame.
    pub(crate) fn color(&mut self, sprite: ColorSprite) {
        let ink = self.ink(sprite.frame, sprite.space);
        let lane = &mut self.frame.lanes.color;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(sprite);
        self.entries.push((Entry::Color(sprite.tile.page), ink));
    }

    /// Adds a marks item drawn from the frame with `payload`.
    pub(crate) fn marks(&mut self, mut marks: FrameMarks, payload: FrameMarkPayload) {
        let ink = self.ink(marks.marks.bounds, marks.marks.space.0);
        marks.marks.payload = self.frame.lanes.mark_payloads.len() as u32;
        self.frame.lanes.mark_payloads.push(payload);
        let lane = &mut self.frame.lanes.marks;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(marks);
        self.entries.push((Entry::Marks, ink));
    }

    /// Opens a group, with its bounds cut to the output.
    ///
    /// Items until the matching [`FrameBuilder::end_group`] draw into it.
    pub(crate) fn begin_group(&mut self, mut group: FrameGroup) {
        let index = self.frame.groups.len() as u32;
        group.bounds = cut(group.bounds, self.output);
        let ink = rounded(group.bounds, self.output);
        self.frame.groups.push(group);
        self.open.push(index);
        self.entries.push((Entry::GroupBegin(index), ink));
    }

    /// Closes the innermost open group.
    pub(crate) fn end_group(&mut self) {
        let index = self.open.pop().expect("a group is open");
        let ink = rounded(self.frame.groups[index as usize].bounds, self.output);
        self.entries.push((Entry::GroupEnd(index), ink));
    }

    /// Adds a backdrop.
    pub(crate) fn backdrop(&mut self, backdrop: FrameBackdrop) {
        let index = self.frame.backdrops.len() as u32;
        let ink = rounded(backdrop.bounds, self.output);
        self.frame.backdrops.push(backdrop);
        self.entries.push((Entry::Backdrop(index), ink));
    }

    /// Adds an external quad.
    pub(crate) fn external(&mut self, external: FrameExternal) {
        let index = self.frame.externals.len() as u32;
        let ink = self.ink(external.quad.bounds, external.quad.space.0);
        self.frame.externals.push(external);
        self.entries.push((Entry::External(index), ink));
    }

    /// Adds a path in a vector pass of its own, clipped by clip slot `clip`.
    ///
    /// The pass region is the path's ink aligned outwards to multiples of 16 and cut to the
    /// output.
    pub(crate) fn path(&mut self, path: FramePath, clip: u32) {
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

/// Returns `[x, y, w, h]` cut to `output`, or empty when it lies wholly outside.
fn cut([x, y, w, h]: [f32; 4], output: DeviceRect) -> [f32; 4] {
    let width = output.size.width as f32;
    let height = output.size.height as f32;
    let left = x.clamp(0.0, width);
    let top = y.clamp(0.0, height);
    let right = (x + w).clamp(left, width);
    let bottom = (y + h).clamp(top, height);
    [left, top, right - left, bottom - top]
}

/// Returns the whole pixels `bounds` touches, cut to `output`.
fn rounded(bounds: [f32; 4], output: DeviceRect) -> DeviceRect {
    let [x, y, w, h] = bounds;
    Rect::from_corners(
        Point::new(x.floor() as i32, y.floor() as i32),
        Point::new((x + w).ceil() as i32, (y + h).ceil() as i32),
    )
    .intersection(output)
    .unwrap_or(Rect::ZERO)
}

/// Returns `ink` aligned outwards to multiples of 16 and cut to `output`.
fn aligned(ink: DeviceRect, output: DeviceRect) -> DeviceRect {
    let left = ink.origin.x.div_euclid(16) * 16;
    let top = ink.origin.y.div_euclid(16) * 16;
    let right = (ink.origin.x + ink.size.width + 15).div_euclid(16) * 16;
    let bottom = (ink.origin.y + ink.size.height + 15).div_euclid(16) * 16;
    Rect::from_corners(Point::new(left, top), Point::new(right, bottom))
        .intersection(output)
        .unwrap_or(Rect::ZERO)
}
