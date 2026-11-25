//! A model of a renderer that applies frames and lists what each one draws.

use std::collections::BTreeMap;
use std::sync::Arc;

use iceblink::{
    BatchKind, ChunkNote, ClipEntry, ColorSprite, FRAME_NO_CLIP, Frame, FrameGroup, FramePaint,
    Instance, ItemSource, Lane, LoweredChunk, Matrix4, PaintKind, PaintRef, Quad,
    RenderCapabilities, Sprite,
};

use crate::validate::{FrameValidator, describe};

/// A model of the renderer of one output: the tables and resident chunks it holds, and the frame
/// check it applies to every frame.
#[derive(Debug, Default)]
pub(crate) struct Replay {
    spaces: Vec<Matrix4>,
    clips: Vec<ClipEntry>,
    paints: Vec<FramePaint>,
    resident: BTreeMap<u64, Arc<LoweredChunk>>,
    validator: FrameValidator,
}

/// A quad a frame draws, resolved through the renderer's tables.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DrawnQuad {
    /// The fill colour, as eight-bit channels, when the fill is solid.
    pub(crate) color: Option<[u8; 4]>,
    /// The device bounding box of the quad's bounds.
    pub(crate) bounds: [f32; 4],
    /// The clip chain, when the quad has one.
    pub(crate) clip: Option<ClipEntry>,
    /// The groups open when the quad is drawn, outermost first.
    pub(crate) groups: Vec<usize>,
}

/// A coverage sprite a frame draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DrawnSprite {
    /// The device bounding box of the sprite's bounds.
    pub(crate) bounds: [f32; 4],
    /// Whether the sprite was drawn from the subpixel lane.
    pub(crate) subpixel: bool,
}

/// Everything one frame draws, in draw order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Picture {
    pub(crate) quads: Vec<DrawnQuad>,
    pub(crate) sprites: Vec<DrawnSprite>,
    /// The colour sprites, at their final position in their local space.
    pub(crate) colors: Vec<ColorSprite>,
    pub(crate) groups: Vec<FrameGroup>,
    /// The clip chain each group is composited through, when it has one.
    pub(crate) group_clips: Vec<Option<ClipEntry>>,
}

impl Picture {
    /// Returns the quads filled with `color`, in draw order.
    pub(crate) fn quads_of(&self, color: [u8; 4]) -> Vec<&DrawnQuad> {
        self.quads
            .iter()
            .filter(|quad| quad.color == Some(color))
            .collect()
    }

    /// Returns the one quad filled with `color`.
    ///
    /// # Panics
    ///
    /// Panics unless exactly one quad has that fill.
    #[track_caller]
    pub(crate) fn quad(&self, color: [u8; 4]) -> &DrawnQuad {
        let found = self.quads_of(color);
        assert_eq!(
            found.len(),
            1,
            "the frame draws {} quads of colour {color:?}, not one",
            found.len()
        );
        found[0]
    }

    /// Returns the position in draw order of the one quad filled with `color`.
    #[track_caller]
    pub(crate) fn position(&self, color: [u8; 4]) -> usize {
        let quad = self.quad(color);
        self.quads
            .iter()
            .position(|other| std::ptr::eq(other, quad))
            .unwrap_or_default()
    }
}

impl Replay {
    /// Returns the model of a renderer that reports `caps` and has drawn nothing.
    pub(crate) fn new(caps: RenderCapabilities) -> Self {
        let mut validator = FrameValidator::new();
        validator.set_capabilities(Some(caps));
        Self {
            validator,
            ..Self::default()
        }
    }

    /// Checks `frame` against the frame contract, applies its notes and tables, and returns what
    /// it draws.
    ///
    /// # Panics
    ///
    /// Panics when the frame breaks the frame contract.
    #[track_caller]
    pub(crate) fn apply(&mut self, frame: &Frame) -> Picture {
        if let Err(violations) = self.validator.check(frame) {
            panic!(
                "frame {} breaks the frame contract:\n{}",
                frame.id.0,
                describe(&violations)
            );
        }

        for note in &frame.chunk_notes {
            match note {
                ChunkNote::Inserted { revision, chunk } => {
                    self.resident.insert(*revision, chunk.clone());
                }
                ChunkNote::Retired { revision } => {
                    self.resident.remove(revision);
                }
                ChunkNote::ResetAll => self.resident.clear(),
            }
        }
        frame.tables.spaces.apply(&mut self.spaces);
        frame.tables.clips.apply(&mut self.clips);
        frame.tables.paints.apply(&mut self.paints);

        self.picture(frame)
    }

    /// Forgets everything the renderer held, as after `FrameOutcome::Recovered`.
    pub(crate) fn device_lost(&mut self) {
        self.spaces.clear();
        self.clips.clear();
        self.paints.clear();
        self.resident.clear();
        self.validator.device_lost();
    }

    /// Returns what `frame` draws, resolved through the tables and resident chunks.
    fn picture(&self, frame: &Frame) -> Picture {
        let quads = self.resolved(frame, &frame.lanes.quads, |chunk| &chunk.quads);
        let mono = self.resolved(frame, &frame.lanes.mono, |chunk| &chunk.mono);
        let subpixel = self.resolved(frame, &frame.lanes.subpixel, |chunk| &chunk.subpixel);
        let colors = self.resolved(frame, &frame.lanes.color, |chunk| &chunk.color);

        let mut picture = Picture {
            groups: frame.groups.clone(),
            group_clips: frame
                .groups
                .iter()
                .map(|group| self.clip(group.clip))
                .collect(),
            ..Picture::default()
        };
        let mut open = Vec::new();
        for batch in &frame.batches {
            match batch.kind {
                BatchKind::Quads => {
                    for position in batch.range.clone() {
                        if let Some(quad) = quads.get(position as usize) {
                            picture.quads.push(self.drawn_quad(quad, &open));
                        }
                    }
                }
                BatchKind::Mono { .. } => {
                    let drawn = batch.range.clone().filter_map(|at| mono.get(at as usize));
                    let drawn = drawn.map(|sprite| self.drawn_sprite(sprite, false));
                    picture.sprites.extend(drawn);
                }
                BatchKind::Subpixel { .. } => {
                    let drawn = batch
                        .range
                        .clone()
                        .filter_map(|at| subpixel.get(at as usize));
                    let drawn = drawn.map(|sprite| self.drawn_sprite(sprite, true));
                    picture.sprites.extend(drawn);
                }
                BatchKind::Color { .. } => {
                    let drawn = batch.range.clone().filter_map(|at| colors.get(at as usize));
                    picture.colors.extend(drawn);
                }
                BatchKind::GroupBegin(index) => open.push(index as usize),
                BatchKind::GroupEnd(_) => {
                    open.pop();
                }
                _ => {}
            }
        }
        picture
    }

    /// Returns every instance of `lane` at its final position, in lane order.
    fn resolved<T: Instance>(
        &self,
        frame: &Frame,
        lane: &Lane<T>,
        array: fn(&LoweredChunk) -> &Vec<T>,
    ) -> Vec<T> {
        lane.instances()
            .filter_map(|source| self.instance(frame, lane, source, array))
            .collect()
    }

    /// Returns the instance `source` refers to, moved by its chunk's offset when it is resident.
    fn instance<T: Instance>(
        &self,
        frame: &Frame,
        lane: &Lane<T>,
        source: ItemSource,
        array: fn(&LoweredChunk) -> &Vec<T>,
    ) -> Option<T> {
        if source.is_transient() {
            return lane.items.get(source.index as usize).copied();
        }
        let drawn = frame.drawn.get(source.chunk as usize)?;
        let chunk = self.resident.get(&drawn.revision)?;
        let instance = array(chunk).get(source.index as usize)?;
        Some(instance.moved(drawn.offset))
    }

    fn drawn_quad(&self, quad: &Quad, open: &[usize]) -> DrawnQuad {
        DrawnQuad {
            color: self.solid(quad.fill),
            bounds: self.device(quad.bounds, quad.space.0),
            clip: self.clip(quad.clip.0),
            groups: open.to_vec(),
        }
    }

    fn drawn_sprite(&self, sprite: &Sprite, subpixel: bool) -> DrawnSprite {
        DrawnSprite {
            bounds: self.device(sprite.bounds, sprite.space),
            subpixel,
        }
    }

    fn solid(&self, paint: PaintRef) -> Option<[u8; 4]> {
        if paint.kind != PaintKind::SOLID {
            return None;
        }
        match self.paints.get(paint.index as usize)? {
            FramePaint::Solid(color) => Some(color.to_rgba8()),
            _ => None,
        }
    }

    fn clip(&self, slot: u32) -> Option<ClipEntry> {
        if slot == FRAME_NO_CLIP {
            return None;
        }
        self.clips.get(slot as usize).cloned()
    }

    /// Returns the device bounding box of `bounds` in frame space `space`.
    fn device(&self, bounds: [f32; 4], space: u32) -> [f32; 4] {
        let matrix = self
            .spaces
            .get(space as usize)
            .copied()
            .unwrap_or(Matrix4::IDENTITY);
        device_box(&matrix, bounds)
    }
}

/// The bounding box of `bounds` mapped through `matrix`, as `[x, y, width, height]`.
fn device_box(matrix: &Matrix4, [x, y, width, height]: [f32; 4]) -> [f32; 4] {
    let corners = [
        (x, y),
        (x + width, y),
        (x, y + height),
        (x + width, y + height),
    ];
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for (cx, cy) in corners {
        let [px, py, _] = matrix.transform_point(cx, cy, 0.0);
        low = [low[0].min(px), low[1].min(py)];
        high = [high[0].max(px), high[1].max(py)];
    }
    [low[0], low[1], high[0] - low[0], high[1] - low[1]]
}
