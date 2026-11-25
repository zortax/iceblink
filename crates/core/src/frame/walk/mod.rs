//! Walking an output's order tree in paint order and emitting what meets the damage.

mod block;
mod chunk;
mod cover;
mod instance;
mod orders;
mod paths;
mod rows;

use iceblink::damage::DamageSet;
use iceblink::{
    ClientId, DeviceRect, EffectId, FRAME_NO_CLIP, Filter, Frame, FrameCounters, FrameGroup,
    OrderEntry, RenderCapabilities, SubpixelOrder, read_extent,
};
use rustc_hash::FxHashMap;

use crate::derive::order::{content_ink, exact_ink};
use crate::frame::filters::frame_filters;
use crate::geometry::bounds::{DeviceBox, INK_LIMIT, of_pixels};
use crate::output::pins::DrawnPins;
use crate::output::renderer::RendererState;
use crate::plan::Emission;
use crate::resident::Revisions;
use crate::scene::Scene;
use crate::shared_content::Content;
use crate::tables::FrameTables;
use crate::work::Work;

use self::block::Pending;
use self::orders::OrderScratch;
use self::rows::Bands;

/// Buffers one frame's walk fills, kept between frames for their capacity.
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    pub(crate) emission: Emission,
    pub(crate) drawn: FxHashMap<u64, u32>,
    pub(crate) shaders: Vec<u32>,
    pub(crate) path_clips: Vec<u32>,
    pending: Vec<Pending>,
    inks_of_items: Vec<Option<DeviceRect>>,
    orders: OrderScratch,
}

impl Scratch {
    /// Empties every buffer, keeping its capacity.
    pub(crate) fn clear(&mut self) {
        self.emission.clear();
        self.drawn.clear();
        self.shaders.clear();
        self.path_clips.clear();
        self.pending.clear();
    }
}

/// The state a walk reads and updates besides the frame.
pub(crate) struct Parts<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) tables: &'a mut FrameTables,
    pub(crate) revisions: &'a mut Revisions,
    pub(crate) renderer: &'a mut RendererState,
    pub(crate) content: &'a mut Content,
    pub(crate) work: &'a mut Work,
    pub(crate) pins: &'a mut DrawnPins,
}

/// What the request and the output allow the frame's glyph runs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Text {
    /// The stripe order subpixel sprites use, when the output may draw them.
    pub(crate) subpixel: Option<SubpixelOrder>,
    /// Device pixels per logical pixel.
    pub(crate) scale: f32,
}

/// Walks the order tree under `root` into `frame`, culled by the frame's draw damage, and returns
/// what the walk counted.
pub(crate) fn run(
    parts: &mut Parts<'_>,
    frame: &mut Frame,
    caps: RenderCapabilities,
    text: Text,
    root: Option<u32>,
    scratch: &mut Scratch,
) -> FrameCounters {
    scratch.clear();

    let draw = frame.damage.draw;
    let client = parts.scene.client;
    let mut walk = Walk {
        scene: parts.scene,
        tables: parts.tables,
        revisions: parts.revisions,
        renderer: parts.renderer,
        content: parts.content,
        work: parts.work,
        pins: parts.pins,
        client,
        frame,
        caps,
        text,
        draw,
        counters: FrameCounters::default(),
        scratch,
        isolated: 0,
        reach: 0,
    };

    if let Some(root) = root {
        walk.node(root, FRAME_NO_CLIP, 0);
    }

    walk.counters
}

/// What walking one output's order tree works on.
pub(crate) struct Walk<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) tables: &'a mut FrameTables,
    pub(crate) revisions: &'a mut Revisions,
    pub(crate) renderer: &'a mut RendererState,
    pub(crate) content: &'a mut Content,
    pub(crate) work: &'a mut Work,
    /// The tiles the output's frames drew.
    pub(crate) pins: &'a mut DrawnPins,
    pub(crate) client: ClientId,
    pub(crate) frame: &'a mut Frame,
    pub(crate) caps: RenderCapabilities,
    pub(crate) text: Text,
    pub(crate) draw: DamageSet,
    pub(crate) counters: FrameCounters,
    pub(crate) scratch: &'a mut Scratch,
    /// How many isolating groups enclose the current node.
    pub(crate) isolated: u32,
    /// How far past the draw damage the filters of the enclosing groups read, in device pixels.
    pub(crate) reach: i32,
}

impl Walk<'_> {
    /// Returns the output's pixels.
    pub(crate) fn frame_bounds(&self) -> DeviceRect {
        let [width, height] = self.frame.size.map(|side| side.min(i32::MAX as u32) as i32);
        DeviceRect::new(
            iceblink::geom::Point::new(0, 0),
            iceblink::geom::Size::new(width, height),
        )
    }

    /// Reports whether an ink meets the frame's draw damage, grown by what the filters of the
    /// enclosing groups read.
    pub(crate) fn meets(&self, ink: Option<DeviceRect>) -> bool {
        ink.is_some_and(|ink| self.meets_rect(ink))
    }

    /// Reports whether `ink` meets the frame's draw damage, as [`Walk::meets`] does.
    pub(crate) fn meets_rect(&self, ink: DeviceRect) -> bool {
        if self.draw.is_full() {
            return true;
        }
        if self.reach == 0 {
            return self.draw.intersects(ink);
        }

        self.reached().any(|rect| rect.intersects(ink))
    }

    /// Returns the draw damage rectangles grown by what the enclosing groups' filters read.
    pub(crate) fn reached(&self) -> impl Iterator<Item = DeviceRect> + '_ {
        let reach = self.reach;
        self.draw
            .rects()
            .iter()
            .map(move |rect| rect.outset(iceblink::geom::Edges::uniform(reach)))
    }

    /// Walks the subtree of the order node at `slot`, drawn through the clip chain `context`.
    pub(crate) fn node(&mut self, slot: u32, context: u32, depth: usize) {
        let Some(record) = self.scene.orders.at(slot) else {
            return;
        };
        if depth > self.scene.orders.span() {
            return;
        }

        let ink = self.scene.node_ink(&record.ink);
        let Some(ink) = ink.filter(|ink| self.meets(Some(*ink))) else {
            self.counters.order_nodes_skipped += 1;
            return;
        };
        self.counters.order_nodes_visited += 1;

        let len = record.entries.len();
        let reach = self.reach;
        let (inner, group) = match record.node.effect {
            Some(effect) => match self.open_effect(slot, effect, context) {
                Some(opened) => opened,
                None => return,
            },
            None => (context, false),
        };

        self.entries(slot, ink, len, inner, depth);

        if group {
            self.flush();
            self.scratch.emission.end_group();
            self.isolated -= 1;
            self.reach = reach;
        }
    }

    /// Walks the `len` entries of the order node at `slot` at `depth`, drawn through the clip
    /// chain `context`, skipping the rows of an ordered node that miss the damage.
    fn entries(&mut self, slot: u32, ink: DeviceRect, len: usize, context: u32, depth: usize) {
        // Each entry is read again, since walking a child borrows the scene.
        let mut bands = Bands::new();
        if self.rows(slot, ink, &mut bands) {
            for row in bands.iter().cloned().flatten() {
                let entry = self.scene.orders.at(slot).and_then(|record| {
                    let index = record.rows.as_ref()?.entry(row)?;
                    record.entries.get(index).copied()
                });
                let Some(entry) = entry else {
                    break;
                };
                self.entry(entry, context, depth);
            }
        } else {
            for index in 0..len {
                let entry = self
                    .scene
                    .orders
                    .at(slot)
                    .and_then(|record| record.entries.get(index).copied());
                let Some(entry) = entry else {
                    break;
                };
                self.entry(entry, context, depth);
            }
        }
    }

    /// Applies effect `effect` of the order node at `slot`, drawn through the clip chain
    /// `context`, and returns the clip chain the node's entries are drawn through and whether it
    /// opened a group for them.
    ///
    /// A group is composited through `context` and its own clip, and its entries are drawn
    /// through no clip of the effects above it.
    ///
    /// Returns `None` when the entries draw nothing.
    #[inline(never)]
    fn open_effect(&mut self, slot: u32, effect: EffectId, context: u32) -> Option<(u32, bool)> {
        let Some(effect) = self.scene.effects.at(effect.slot()) else {
            return Some((context, false));
        };

        let effect = effect.node.clone();
        let clip = effect
            .clip
            .and_then(|clip| self.scene.clips.at(clip.slot()))
            .map_or(FRAME_NO_CLIP, |clip| clip.links.frame);
        if !effect.isolates() {
            return Some((self.tables.combine(context, clip), false));
        }
        if effect.opacity <= 0.0 {
            return None;
        }

        // A carried group's stored ink can be wider than what it writes. A nested group is cut
        // by the source of the group around it, which holds its ink.
        let ink = exact_ink(self.scene, slot)
            .and_then(|ink| match self.isolated {
                0 => ink.intersection(self.frame_bounds()),
                _ => Some(ink),
            })
            .unwrap_or(iceblink::geom::Rect::ZERO);

        // The source is rounded outwards, so it holds the written region rounded the same way.
        let source = content_ink(self.scene, slot)
            .and_then(|content| {
                DeviceBox::of(read_extent(of_pixels(content).rect(), &effect.filters))
                    .pixels(INK_LIMIT)
            })
            .map_or(ink, |source| match ink.is_empty() {
                true => source,
                false => source.union(ink),
            });

        let filters = frame_filters(self.scene, &effect.filters);
        self.note_filter_shaders(&filters);
        self.reach = self.reach.saturating_add(chain_reach(&effect.filters));
        self.frame.groups.push(FrameGroup {
            bounds: of_pixels(ink).rect(),
            source: of_pixels(source).rect(),
            clip: self.tables.combine(context, clip),
            opacity: effect.opacity.clamp(0.0, 1.0),
            blend: effect.blend,
            filters,
        });

        self.flush();
        self.scratch
            .emission
            .begin_reading_group(ink, chain_reach(&effect.filters));
        self.counters.groups += 1;
        self.isolated += 1;
        // The clips above reach the group's content through its composite.
        Some((FRAME_NO_CLIP, true))
    }

    /// Walks entry `entry` of an order node at `depth`, drawn through the clip chain `context`.
    fn entry(&mut self, entry: OrderEntry, context: u32, depth: usize) {
        match entry {
            OrderEntry::Chunk(key) => self.chunk(key, context),
            OrderEntry::Node(id) => self.node(id.slot(), context, depth + 1),
        }
    }

    /// Notes the shaders a filter chain draws with.
    pub(crate) fn note_filter_shaders(&mut self, filters: &[iceblink::FrameFilter]) {
        for filter in filters {
            if let iceblink::FrameFilter::Shader { shader, .. } = filter {
                self.scratch.shaders.push(*shader);
            }
        }
    }
}

/// Returns how far a filter chain reads past the pixels it writes, on its widest side, in whole
/// device pixels.
fn chain_reach(filters: &[Filter]) -> i32 {
    let [x, y, width, height] = read_extent([0.0; 4], filters);
    let widest = (-x).max(-y).max(width + x).max(height + y);
    widest.ceil().clamp(0.0, i32::MAX as f32) as i32
}
