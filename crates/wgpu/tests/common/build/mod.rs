//! Frames built by hand, as a simple compositor would build them.
//!
//! Items are pushed in paint order. Consecutive instances of one kind that bind the same page or
//! shader share a batch, so paint order holds for any overlap. Tables are written as whole
//! replacements on the first frame and as changed slots after it.

mod batches;
mod items;
mod resident;

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Backdrop, BlendMode, BorderStyle, ClipEntry, ClipRef, Color, DeviceRect, FRAME_NO_CLIP, Filter,
    Frame, FrameBackdrop, FrameFilter, FrameGroup, FramePaint, Matrix4, PageOp, PaintKind,
    PaintRef, ParamsBlock, Quad, ROUND, ShaderOp, SpaceRef, read_extent,
};

use self::batches::Entry;
use super::device::SIDE;

/// Returns `[x, y, width, height]`.
pub(crate) fn rect(x: f32, y: f32, width: f32, height: f32) -> [f32; 4] {
    [x, y, width, height]
}

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

/// Returns the colour of eight-bit straight channels, premultiplied.
pub(crate) fn straight(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
    let [r, g, b, a] = [red, green, blue, alpha].map(|channel| f32::from(channel) / 255.0);
    Color::from_straight(r, g, b, a)
}

/// Returns an opaque colour from eight-bit channels.
pub(crate) fn opaque(red: u8, green: u8, blue: u8) -> Color {
    Color([
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        1.0,
    ])
}

/// Returns a quad over `bounds` filled with `fill`, with no border, clip or space beyond the first.
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

/// Adds a quad over `bounds` filled with the opaque colour `color`.
pub(crate) fn quad(builder: &mut FrameBuilder, bounds: [f32; 4], color: [u8; 3]) {
    let paint = builder.solid(opaque(color[0], color[1], color[2]));
    builder.quad(filled(bounds, paint));
}

/// Returns a source-over group over `bounds` grown by what `filters` read.
///
/// # Panics
///
/// Panics on a shader filter.
pub(crate) fn group(bounds: [f32; 4], opacity: f32, filters: &[Filter]) -> FrameGroup {
    FrameGroup {
        bounds: read_extent(bounds, filters),
        source: read_extent(bounds, filters),
        clip: FRAME_NO_CLIP,
        opacity,
        blend: BlendMode::NORMAL,
        filters: filters.iter().map(frame_filter).collect(),
    }
}

/// Returns a backdrop over `bounds` that blurs what is beneath it by `deviation`.
pub(crate) fn frosted(bounds: [f32; 4], deviation: f32) -> FrameBackdrop {
    let filters = [Filter::Blur(deviation)];
    FrameBackdrop {
        backdrop: Backdrop {
            bounds,
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds,
        source: read_extent(bounds, &filters),
        filters: filters.iter().map(frame_filter).collect(),
    }
}

/// Returns the frame form of `filter`.
///
/// # Panics
///
/// Panics on a shader filter.
pub(crate) fn frame_filter(filter: &Filter) -> FrameFilter {
    match *filter {
        Filter::Blur(std_dev) => FrameFilter::Blur(std_dev),
        Filter::DropShadow {
            offset,
            std_dev,
            color,
        } => FrameFilter::DropShadow {
            offset,
            std_dev,
            color,
        },
        Filter::ColorMatrix(matrix) => FrameFilter::ColorMatrix(matrix),
        Filter::Opacity(opacity) => FrameFilter::Opacity(opacity),
        Filter::Shader { .. } => panic!("a shader filter refers to a slot the builder cannot know"),
    }
}

/// A frame under construction.
pub(crate) struct FrameBuilder {
    frame: Frame,
    /// The batch entries in paint order, each with its ink.
    entries: Vec<(Entry, DeviceRect)>,
    /// The open groups, innermost last.
    open: Vec<u32>,
    spaces: Vec<Matrix4>,
    paints: Vec<FramePaint>,
    clips: Vec<ClipEntry>,
    params: Vec<ParamsBlock>,
    /// Whether the frame replaces every table.
    replace: bool,
    /// The output rectangle, which inks are cut to.
    output: DeviceRect,
}

impl FrameBuilder {
    /// Returns a first frame: every table is replaced, the damage is the whole output, and space
    /// slot 0 is the identity.
    pub(crate) fn new() -> Self {
        let mut frame = Frame {
            size: [SIDE, SIDE],
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
            output: whole(),
        }
    }

    /// Returns a first frame, as [`FrameBuilder::new`] does, for an output of `width` by
    /// `height`.
    pub(crate) fn sized(width: u32, height: u32) -> Self {
        let mut builder = Self::new();
        builder.resize(width, height);
        builder
    }

    /// Sets the output size of the frame under construction, which batch inks are cut to.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.frame.size = [width, height];
        self.output = Rect::new(Point::new(0, 0), Size::new(width as i32, height as i32));
    }

    /// Returns a later frame that changes no table unless it writes a slot.
    pub(crate) fn next() -> Self {
        let mut builder = Self::new();
        builder.replace = false;
        builder.spaces.clear();
        builder
    }

    /// Sets the frame's id.
    pub(crate) fn id(mut self, id: u64) -> Self {
        self.frame.id = iceblink::FrameId(id);
        self
    }

    /// Sets the frame's draw and present damage.
    pub(crate) fn damage(mut self, damage: DamageSet) -> Self {
        self.frame.damage.draw = damage;
        self.frame.damage.present = damage;
        self
    }

    /// Returns the frame under construction, for fields the builder has no method for.
    pub(crate) fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    /// Adds a paint and returns a reference to it.
    pub(crate) fn paint(&mut self, paint: FramePaint) -> PaintRef {
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
    pub(crate) fn solid(&mut self, color: Color) -> PaintRef {
        self.paint(FramePaint::Solid(color))
    }

    /// Writes the paint slot `slot` of a later frame.
    pub(crate) fn set_paint(&mut self, slot: u32, paint: FramePaint) {
        self.frame.tables.paints.changed.push((slot, paint));
    }

    /// Adds a space and returns its slot.
    pub(crate) fn space(&mut self, matrix: Matrix4) -> u32 {
        self.spaces.push(matrix);
        self.spaces.len() as u32 - 1
    }

    /// Writes the space slot `slot` of a later frame.
    pub(crate) fn set_space(&mut self, slot: u32, matrix: Matrix4) {
        self.frame.tables.spaces.changed.push((slot, matrix));
    }

    /// Adds a clip and returns its slot.
    pub(crate) fn clip(&mut self, entry: ClipEntry) -> ClipRef {
        self.clips.push(entry);
        ClipRef(self.clips.len() as u32 - 1)
    }

    /// Adds a parameter block and returns its slot.
    pub(crate) fn params(&mut self, block: ParamsBlock) -> u32 {
        self.params.push(block);
        self.params.len() as u32 - 1
    }

    /// Adds page work.
    pub(crate) fn page(&mut self, op: PageOp) {
        self.frame.resources.pages.push(op);
    }

    /// Adds shader work.
    pub(crate) fn shader(&mut self, op: ShaderOp) {
        self.frame.resources.shaders.push(op);
    }

    /// Returns the finished frame.
    pub(crate) fn finish(mut self) -> Frame {
        assert!(self.open.is_empty(), "every group is closed");

        let tables = &mut self.frame.tables;
        if self.replace {
            tables.spaces.replace = true;
            tables.spaces.all = self.spaces;
            tables.paints.replace = true;
            tables.paints.all = self.paints;
            tables.clips.replace = true;
            tables.clips.all = self.clips;
            tables.params.replace = true;
            tables.params.all = self.params;
        }

        self.frame.batches = batches::batches(self.entries);
        self.frame
    }
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}
