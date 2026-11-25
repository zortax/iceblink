//! Frames built by hand, as a simple compositor would build them.

mod batches;
mod ink;
mod items;
mod layers;
mod resident;
mod values;

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    ClipEntry, ClipRef, Color, DeviceRect, Frame, FramePaint, Matrix4, PageOp, PaintKind, PaintRef,
    ParamsBlock, ShaderOp,
};

use self::batches::{Entry, batches};

pub use self::values::{area, filled, frosted, group, opaque, rect, straight};

/// A frame under construction.
pub struct FrameBuilder {
    frame: Frame,
    entries: Vec<(Entry, DeviceRect)>,
    /// The groups open, innermost last.
    open: Vec<u32>,
    spaces: Vec<Matrix4>,
    paints: Vec<FramePaint>,
    clips: Vec<ClipEntry>,
    params: Vec<ParamsBlock>,
    /// Whether the frame replaces every table.
    replace: bool,
    output: DeviceRect,
}

impl FrameBuilder {
    /// Returns a first frame for an output of `size`: every table is replaced, the damage is the
    /// whole output, and space slot 0 is the identity.
    pub fn new(size: [u32; 2]) -> Self {
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
            entries: Vec::new(),
            open: Vec::new(),
            spaces: vec![Matrix4::IDENTITY],
            paints: Vec::new(),
            clips: Vec::new(),
            params: Vec::new(),
            replace: true,
            output: Rect::new(Point::new(0, 0), Size::new(size[0] as i32, size[1] as i32)),
        }
    }

    /// Returns a later frame for an output of `size` that changes no table unless it writes a
    /// slot.
    pub fn next(size: [u32; 2]) -> Self {
        let mut builder = Self::new(size);
        builder.replace = false;
        builder.spaces.clear();
        builder
    }

    /// Sets the frame's id.
    pub fn id(mut self, id: u64) -> Self {
        self.frame.id = iceblink::FrameId(id);
        self
    }

    /// Sets the frame's draw and present damage.
    pub fn damage(mut self, damage: DamageSet) -> Self {
        self.frame.damage.draw = damage;
        self.frame.damage.present = damage;
        self
    }

    /// Returns the frame under construction, for fields the builder has no method for.
    pub fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    /// Adds a paint and returns a reference to it.
    pub fn paint(&mut self, paint: FramePaint) -> PaintRef {
        let kind = match &paint {
            FramePaint::None => PaintKind::NONE,
            FramePaint::Solid(_) => PaintKind::SOLID,
            FramePaint::Gradient(_) => PaintKind::GRADIENT,
            FramePaint::Sides(_) => PaintKind::SIDES,
        };
        let index = self.paints.len() as u32;
        self.paints.push(paint);
        PaintRef {
            kind,
            _pad: [0; 3],
            index,
        }
    }

    /// Adds a solid paint.
    pub fn solid(&mut self, color: Color) -> PaintRef {
        self.paint(FramePaint::Solid(color))
    }

    /// Writes the paint slot `slot` of a later frame.
    pub fn set_paint(&mut self, slot: u32, paint: FramePaint) {
        self.frame.tables.paints.changed.push((slot, paint));
    }

    /// Adds a space and returns its slot.
    pub fn space(&mut self, matrix: Matrix4) -> u32 {
        self.spaces.push(matrix);
        self.spaces.len() as u32 - 1
    }

    /// Writes the space slot `slot` of a later frame.
    pub fn set_space(&mut self, slot: u32, matrix: Matrix4) {
        self.frame.tables.spaces.changed.push((slot, matrix));
    }

    /// Adds a clip and returns its slot.
    pub fn clip(&mut self, entry: ClipEntry) -> ClipRef {
        self.clips.push(entry);
        ClipRef(self.clips.len() as u32 - 1)
    }

    /// Adds a parameter block and returns its slot.
    pub fn params(&mut self, block: ParamsBlock) -> u32 {
        self.params.push(block);
        self.params.len() as u32 - 1
    }

    /// Adds page work.
    pub fn page(&mut self, op: PageOp) {
        self.frame.resources.pages.push(op);
    }

    /// Adds shader work.
    pub fn shader(&mut self, op: ShaderOp) {
        self.frame.resources.shaders.push(op);
    }

    /// Returns the finished frame, with its items in batches in the order they were added.
    ///
    /// # Panics
    ///
    /// Panics when a group is still open.
    pub fn finish(mut self) -> Frame {
        assert!(self.open.is_empty(), "every group is closed");

        if self.replace {
            let tables = &mut self.frame.tables;
            tables.spaces.replace = true;
            tables.spaces.all = self.spaces;
            tables.paints.replace = true;
            tables.paints.all = self.paints;
            tables.clips.replace = true;
            tables.clips.all = self.clips;
            tables.params.replace = true;
            tables.params.all = self.params;
        }

        self.frame.batches = batches(self.entries);
        self.frame
    }
}
