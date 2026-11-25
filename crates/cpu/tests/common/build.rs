//! Frames built by hand: every table replaced, one batch per instance, in paint order.

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Batch, BatchKind, BorderStyle, ClipEntry, ClipRef, Color, ColorSprite, DeviceRect,
    FRAME_NO_CLIP, Frame, FrameBackdrop, FrameExternal, FrameGroup, FrameMarkPayload, FrameMarks,
    FramePaint, FramePath, FrameShadow, ItemSource, Line, Marks, Matrix4, PageFormat, PageOp,
    PaintKind, PaintRef, PassKey, PlannedPass, PlannedPath, Quad, ROUND, SpaceRef, Sprite,
};

use crate::common::SIDE;

/// Returns the whole output as a device rectangle.
pub(crate) fn whole() -> DeviceRect {
    Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32))
}

/// Returns the damage of the one rectangle `[x, y, width, height]`.
pub(crate) fn area(x: i32, y: i32, width: i32, height: i32) -> DamageSet {
    let mut set = DamageSet::new();
    set.absorb(Rect::new(Point::new(x, y), Size::new(width, height)));
    set
}

/// Returns a quad over `bounds` filled with `fill`, in space 0 with no clip.
pub(crate) fn filled(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// A frame under construction.
pub(crate) struct FrameBuilder {
    frame: Frame,
    spaces: Vec<Matrix4>,
    paints: Vec<FramePaint>,
    clips: Vec<ClipEntry>,
}

impl FrameBuilder {
    /// Returns a frame that redraws the whole output, with space slot 0 the identity.
    pub(crate) fn new() -> Self {
        Self::sized([SIDE, SIDE])
    }

    /// Returns a frame of an output of `size`, as [`FrameBuilder::new`] does.
    pub(crate) fn sized(size: [u32; 2]) -> Self {
        let mut frame = Frame {
            size,
            scale: 1.0,
            opaque: true,
            ..Frame::default()
        };
        frame.damage.draw = DamageSet::full();
        frame.damage.present = DamageSet::full();
        Self {
            frame,
            spaces: vec![Matrix4::IDENTITY],
            paints: Vec::new(),
            clips: Vec::new(),
        }
    }

    /// Adds a space with world matrix `matrix` and returns its slot.
    pub(crate) fn space(&mut self, matrix: Matrix4) -> u32 {
        self.spaces.push(matrix);
        self.spaces.len() as u32 - 1
    }

    /// Adds `paint` and returns a reference to it.
    pub(crate) fn paint(&mut self, paint: FramePaint) -> PaintRef {
        let kind = match &paint {
            FramePaint::None => PaintKind::NONE,
            FramePaint::Solid(_) => PaintKind::SOLID,
            FramePaint::Gradient(_) => PaintKind::GRADIENT,
            FramePaint::Sides(_) => PaintKind::SIDES,
        };
        self.paints.push(paint);
        PaintRef {
            kind,
            _pad: [0; 3],
            index: self.paints.len() as u32 - 1,
        }
    }

    /// Adds a solid paint of the premultiplied colour `rgba`.
    pub(crate) fn solid(&mut self, rgba: [f32; 4]) -> PaintRef {
        self.paint(FramePaint::Solid(Color(rgba)))
    }

    /// Adds a clip chain and returns its slot.
    pub(crate) fn clip(&mut self, entry: ClipEntry) -> u32 {
        self.clips.push(entry);
        self.clips.len() as u32 - 1
    }

    /// Sets the frame's draw and present damage.
    pub(crate) fn damage(&mut self, damage: DamageSet) -> &mut Self {
        self.frame.damage.draw = damage;
        self.frame.damage.present = damage;
        self
    }

    /// Draws `quad`.
    pub(crate) fn quad(&mut self, quad: Quad) -> &mut Self {
        let lane = &mut self.frame.lanes.quads;
        push(&mut self.frame.batches, &mut lane.sources, BatchKind::Quads);
        lane.items.push(quad);
        self
    }

    /// Draws `shadow`.
    pub(crate) fn shadow(&mut self, shadow: FrameShadow) -> &mut Self {
        let lane = &mut self.frame.lanes.shadows;
        push(
            &mut self.frame.batches,
            &mut lane.sources,
            BatchKind::Shadows,
        );
        lane.items.push(shadow);
        self
    }

    /// Draws `line`.
    pub(crate) fn line(&mut self, line: Line) -> &mut Self {
        let lane = &mut self.frame.lanes.lines;
        push(&mut self.frame.batches, &mut lane.sources, BatchKind::Lines);
        lane.items.push(line);
        self
    }

    /// Creates page `page` of `format` and `size` holding `texels` row by row, with every level
    /// down to one texel and only level 0 written.
    pub(crate) fn page(
        &mut self,
        page: u32,
        format: PageFormat,
        size: [u32; 2],
        texels: Vec<u8>,
    ) -> &mut Self {
        let stride = texels.len() as u32 / size[1];
        let levels = 32 - size[0].max(size[1]).leading_zeros();
        self.frame.resources.pages.push(PageOp::Create {
            page,
            format,
            size,
            levels,
        });
        self.upload(page, 0, size, texels.into(), stride);
        self
    }

    /// Writes level `level` of `page` whole.
    pub(crate) fn upload(
        &mut self,
        page: u32,
        level: u32,
        size: [u32; 2],
        bytes: Arc<[u8]>,
        stride: u32,
    ) -> &mut Self {
        self.frame.resources.pages.push(PageOp::Upload {
            page,
            level,
            rect: Rect::new(Point::new(0, 0), Size::new(size[0] as i32, size[1] as i32)),
            bytes,
            offset: 0,
            stride,
        });
        self
    }

    /// Draws `sprite` from a single-channel page.
    pub(crate) fn mono(&mut self, sprite: Sprite) -> &mut Self {
        let lane = &mut self.frame.lanes.mono;
        push(
            &mut self.frame.batches,
            &mut lane.sources,
            BatchKind::Mono {
                page: sprite.tile.page,
            },
        );
        lane.items.push(sprite);
        self
    }

    /// Draws `sprite` from a subpixel page.
    pub(crate) fn subpixel(&mut self, sprite: Sprite) -> &mut Self {
        let lane = &mut self.frame.lanes.subpixel;
        push(
            &mut self.frame.batches,
            &mut lane.sources,
            BatchKind::Subpixel {
                page: sprite.tile.page,
            },
        );
        lane.items.push(sprite);
        self
    }

    /// Draws `sprite` from a colour page.
    pub(crate) fn color(&mut self, sprite: ColorSprite) -> &mut Self {
        let lane = &mut self.frame.lanes.color;
        push(
            &mut self.frame.batches,
            &mut lane.sources,
            BatchKind::Color {
                page: sprite.tile.page,
            },
        );
        lane.items.push(sprite);
        self
    }

    /// Draws `marks`, which draw transient payload `payload`.
    pub(crate) fn marks(&mut self, marks: FrameMarks, payload: FrameMarkPayload) -> &mut Self {
        let lane = &mut self.frame.lanes.marks;
        push(&mut self.frame.batches, &mut lane.sources, BatchKind::Marks);
        lane.items.push(FrameMarks {
            marks: Marks {
                payload: self.frame.lanes.mark_payloads.len() as u32,
                ..marks.marks
            },
            ..marks
        });
        self.frame.lanes.mark_payloads.push(payload);
        self
    }

    /// Draws `path` in a pass of its own over `region`, keyed by `key`.
    pub(crate) fn path(&mut self, path: FramePath, region: DeviceRect, key: PassKey) -> &mut Self {
        let index = self.frame.paths.len() as u32;
        let item = self.frame.vector_plan.items.len() as u32;
        let ink = Rect::new(
            Point::new(
                path.ink.left() - region.left(),
                path.ink.top() - region.top(),
            ),
            path.ink.size,
        );
        self.frame.paths.push(path);
        self.frame.vector_plan.items.push(PlannedPath {
            path: index,
            residual: FRAME_NO_CLIP,
            clip: FRAME_NO_CLIP,
            ink,
        });
        let pass = self.frame.vector_plan.passes.len() as u32;
        self.frame.vector_plan.passes.push(PlannedPass {
            items: item..item + 1,
            region,
            clip: FRAME_NO_CLIP,
            instanced: false,
            clamped: false,
            overhangs: false,
            key,
        });
        self.marker(BatchKind::Paths { pass })
    }

    /// Opens `group`, whose content follows until [`FrameBuilder::end`].
    pub(crate) fn begin(&mut self, group: FrameGroup) -> &mut Self {
        let index = self.frame.groups.len() as u32;
        self.frame.groups.push(group);
        self.marker(BatchKind::GroupBegin(index))
    }

    /// Closes the group opened last.
    pub(crate) fn end(&mut self) -> &mut Self {
        let open = self
            .frame
            .batches
            .iter()
            .rev()
            .scan(0i32, |depth, batch| {
                Some(match batch.kind {
                    BatchKind::GroupEnd(_) => {
                        *depth += 1;
                        None
                    }
                    BatchKind::GroupBegin(index) if *depth == 0 => Some(index),
                    BatchKind::GroupBegin(_) => {
                        *depth -= 1;
                        None
                    }
                    _ => None,
                })
            })
            .flatten()
            .next()
            .expect("a group is open");
        self.marker(BatchKind::GroupEnd(open))
    }

    /// Applies `backdrop` to what is drawn so far.
    pub(crate) fn backdrop(&mut self, backdrop: FrameBackdrop) -> &mut Self {
        let index = self.frame.backdrops.len() as u32;
        self.frame.backdrops.push(backdrop);
        self.marker(BatchKind::Backdrop(index))
    }

    /// Draws `external`.
    pub(crate) fn external(&mut self, external: FrameExternal) -> &mut Self {
        let index = self.frame.externals.len() as u32;
        self.frame.externals.push(external);
        self.marker(BatchKind::External(index))
    }

    /// Returns a copy of the frame so far, with every table replaced.
    pub(crate) fn clone_frame(&self) -> Frame {
        let frame = &self.frame;
        let copy = Frame {
            id: frame.id,
            output: frame.output,
            size: frame.size,
            scale: frame.scale,
            opaque: frame.opaque,
            damage: frame.damage,
            shifts: frame.shifts.clone(),
            chunk_notes: frame.chunk_notes.clone(),
            drawn: frame.drawn.clone(),
            externals: frame.externals.clone(),
            backdrops: frame.backdrops.clone(),
            groups: frame.groups.clone(),
            batches: frame.batches.clone(),
            resources: frame.resources.clone(),
            ..Frame::default()
        };
        let mut builder = Self {
            frame: copy,
            spaces: self.spaces.clone(),
            paints: self.paints.clone(),
            clips: self.clips.clone(),
        };
        let lanes = &self.frame.lanes;
        let into = &mut builder.frame.lanes;
        into.quads.items.clone_from(&lanes.quads.items);
        into.quads.sources.clone_from(&lanes.quads.sources);
        into.shadows.items.clone_from(&lanes.shadows.items);
        into.shadows.sources.clone_from(&lanes.shadows.sources);
        into.lines.items.clone_from(&lanes.lines.items);
        into.lines.sources.clone_from(&lanes.lines.sources);
        into.mono.items.clone_from(&lanes.mono.items);
        into.mono.sources.clone_from(&lanes.mono.sources);
        into.subpixel.items.clone_from(&lanes.subpixel.items);
        into.subpixel.sources.clone_from(&lanes.subpixel.sources);
        into.color.items.clone_from(&lanes.color.items);
        into.color.sources.clone_from(&lanes.color.sources);
        into.marks.items.clone_from(&lanes.marks.items);
        into.marks.sources.clone_from(&lanes.marks.sources);
        into.mark_payloads.clone_from(&lanes.mark_payloads);
        builder.finish()
    }

    /// Returns the frame, with every table replaced.
    pub(crate) fn finish(mut self) -> Frame {
        let tables = &mut self.frame.tables;
        tables.spaces.replace = true;
        tables.spaces.all = self.spaces;
        tables.paints.replace = true;
        tables.paints.all = self.paints;
        tables.clips.replace = true;
        tables.clips.all = self.clips;
        self.frame
    }

    /// Sets the frame's id.
    pub(crate) fn id(&mut self, id: u64) -> &mut Self {
        self.frame.id = iceblink::FrameId(id);
        self
    }

    fn marker(&mut self, kind: BatchKind) -> &mut Self {
        let ink = self.ink();
        self.frame.batches.push(Batch {
            kind,
            range: 0..0,
            ink,
        });
        self
    }

    fn ink(&self) -> DeviceRect {
        let [width, height] = self.frame.size;
        Rect::new(Point::new(0, 0), Size::new(width as i32, height as i32))
    }
}

fn push(batches: &mut Vec<Batch>, sources: &mut Vec<ItemSource>, kind: BatchKind) {
    // Batches may reach the whole of any output a test draws.
    let ink = Rect::new(Point::new(0, 0), Size::new(1 << 14, 1 << 14));
    let position = sources.len() as u32;
    let index = sources
        .iter()
        .filter(|source| source.is_transient())
        .count() as u32;
    sources.push(ItemSource::transient(index));
    batches.push(Batch {
        kind,
        range: position..position + 1,
        ink,
    });
}
