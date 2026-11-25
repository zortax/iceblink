//! Adding lowered instances to the frame's lanes and the planner's emission.

use core::ops::Range;

use iceblink::{
    DeviceRect, Instance, ItemSource, Lane as FrameLane, LocalRect, LoweredChunk, MarkFlags,
};

use crate::frame::walk::Walk;
use crate::frame::walk::cover::cover;
use crate::geometry::bounds::{INK_LIMIT, map};
use crate::plan::{Instance as Planned, LaneKind};
use crate::scene::chunk::{Lane, LaneOrders};

/// Where the instances of one lowered item are drawn.
pub(super) struct Placed<'a> {
    pub(super) chunk: &'a LoweredChunk,
    /// The index of the chunk's revision in `Frame::drawn`, when the renderer keeps it resident.
    pub(super) resident: Option<u32>,
    /// The offset added to the lowered instances.
    pub(super) delta: [f32; 2],
    /// The item's device ink.
    pub(super) ink: DeviceRect,
    /// Whether the item lowered to this one instance, whose ink is the item's.
    pub(super) alone: bool,
    /// The local orders of the chunk's instances.
    pub(super) orders: &'a LaneOrders,
}

impl Walk<'_> {
    /// Adds the instances `start..start + len` of a lowered array.
    ///
    /// Consecutive sprites with one page, one space and one local order are added as one run,
    /// whose ink is their union. Every other instance is added on its own.
    pub(super) fn instances(&mut self, lane: Lane, start: u32, len: u32, placed: &Placed<'_>) {
        if len < 2 || !matches!(lane, Lane::Mono | Lane::Subpixel | Lane::Color) {
            for index in start..start + len {
                self.instance(lane, index, placed);
            }
            return;
        }

        let chunk = placed.chunk;
        let page = |index: u32| {
            let at = index as usize;
            match lane {
                Lane::Mono => chunk.mono.get(at).map(|s| (s.tile.page, s.space)),
                Lane::Subpixel => chunk.subpixel.get(at).map(|s| (s.tile.page, s.space)),
                _ => chunk.color.get(at).map(|s| (s.tile.page, s.space)),
            }
        };

        let end = start + len;
        let mut first = start;
        while first < end {
            let key = (page(first), placed.orders.of(lane, first));
            let mut last = first + 1;
            while last < end && (page(last), placed.orders.of(lane, last)) == key {
                last += 1;
            }
            match key.0 {
                Some((page, space)) if last - first > 1 => {
                    self.sprite_run(lane, first..last, [page, space], placed);
                }
                _ => {
                    for index in first..last {
                        self.instance(lane, index, placed);
                    }
                }
            }
            first = last;
        }
    }

    /// Adds the sprites `indices` of a lowered sprite array, all on page `page` in frame space
    /// `space`, as one run.
    ///
    /// The run's ink is the device box of the union of their bounds. When that box reaches outside
    /// the item's ink, the sprites are added one by one instead.
    fn sprite_run(
        &mut self,
        lane: Lane,
        indices: Range<u32>,
        [page, space]: [u32; 2],
        placed: &Placed<'_>,
    ) {
        let index = indices.start;
        let range = indices.start as usize..indices.end as usize;
        let chunk = placed.chunk;
        let bounds = match lane {
            Lane::Mono => union(chunk.mono[range.clone()].iter().map(|s| s.bounds)),
            Lane::Subpixel => union(chunk.subpixel[range.clone()].iter().map(|s| s.bounds)),
            _ => union(chunk.color[range.clone()].iter().map(|s| s.bounds)),
        };

        let [x, y, width, height] = bounds;
        let moved = [x + placed.delta[0], y + placed.delta[1], width, height];
        // Sprites the item's clips cut away are left out one by one, so a run holds only
        // sprites that lie wholly inside the item's ink.
        let whole = self
            .tables
            .spaces
            .get(space)
            .and_then(|world| map(world, moved).pixels(INK_LIMIT));
        let Some(ink) = whole.filter(|whole| placed.ink.contains_rect(*whole)) else {
            for index in indices {
                self.instance(lane, index, placed);
            }
            return;
        };

        let lanes = &mut self.frame.lanes;
        let (source, kind) = match lane {
            Lane::Mono => (
                run(&mut lanes.mono, &chunk.mono[range], placed, index),
                LaneKind::Mono,
            ),
            Lane::Subpixel => (
                run(&mut lanes.subpixel, &chunk.subpixel[range], placed, index),
                LaneKind::Subpixel,
            ),
            _ => (
                run(&mut lanes.color, &chunk.color[range], placed, index),
                LaneKind::Color,
            ),
        };
        let planned = Planned::new(kind, source, ink).keyed([page, 0]);
        self.pend(planned, None, placed.orders.of(lane, index));
    }

    /// Adds instance `index` of a lowered array, as a resident source or a transient copy.
    pub(super) fn instance(&mut self, lane: Lane, index: u32, placed: &Placed<'_>) {
        let at = index as usize;
        let chunk = placed.chunk;
        let lanes = &mut self.frame.lanes;
        let planned = match lane {
            Lane::Quads => {
                let Some(quad) = chunk.quads.get(at).map(|quad| quad.moved(placed.delta)) else {
                    return;
                };
                let source = source(&mut lanes.quads, quad, placed, index);
                let planned = Planned::new(LaneKind::Quads, source, placed.ink);
                let cover = placed
                    .ink
                    .intersection(self.frame_bounds())
                    .and_then(|visible| cover(self.tables, &quad, visible));
                self.pend(planned, cover, placed.orders.of(lane, index));
                return;
            }
            Lane::ShaderQuads => {
                let Some(quad) = chunk.shader_quads.get(at).map(|q| q.moved(placed.delta)) else {
                    return;
                };
                self.scratch.shaders.push(quad.shader);
                let source = source(&mut lanes.shader_quads, quad, placed, index);
                Planned::new(LaneKind::ShaderQuads, source, placed.ink)
                    .keyed([quad.shader, quad.params])
            }
            Lane::Shadows => {
                let Some(shadow) = chunk.shadows.get(at).map(|s| s.moved(placed.delta)) else {
                    return;
                };
                let source = source(&mut lanes.shadows, shadow, placed, index);
                Planned::new(LaneKind::Shadows, source, placed.ink)
            }
            Lane::Lines => {
                let Some(line) = chunk.lines.get(at).map(|line| line.moved(placed.delta)) else {
                    return;
                };
                let source = source(&mut lanes.lines, line, placed, index);
                Planned::new(LaneKind::Lines, source, placed.ink)
            }
            Lane::Mono | Lane::Subpixel => {
                let (array, kind) = if lane == Lane::Mono {
                    (&chunk.mono, LaneKind::Mono)
                } else {
                    (&chunk.subpixel, LaneKind::Subpixel)
                };
                let Some(sprite) = array.get(at).map(|s| s.moved(placed.delta)) else {
                    return;
                };
                let Some(ink) = self.own_ink(sprite.bounds, sprite.space, placed) else {
                    return;
                };
                let lanes = &mut self.frame.lanes;
                let lane = if lane == Lane::Mono {
                    &mut lanes.mono
                } else {
                    &mut lanes.subpixel
                };
                let source = source(lane, sprite, placed, index);
                Planned::new(kind, source, ink).keyed([sprite.tile.page, 0])
            }
            Lane::Color => {
                let Some(sprite) = chunk.color.get(at).map(|s| s.moved(placed.delta)) else {
                    return;
                };
                let Some(ink) = self.own_ink(sprite.bounds, sprite.space, placed) else {
                    return;
                };
                let source = source(&mut self.frame.lanes.color, sprite, placed, index);
                Planned::new(LaneKind::Color, source, ink).keyed([sprite.tile.page, 0])
            }
            Lane::Marks => {
                let Some(mut marks) = chunk.marks.get(at).map(|m| m.moved(placed.delta)) else {
                    return;
                };
                let union = marks.marks.flags.contains(MarkFlags::UNION);
                if placed.resident.is_none() {
                    let payload = chunk.mark_payloads.get(marks.marks.payload as usize);
                    let Some(payload) = payload.cloned() else {
                        return;
                    };
                    marks.marks.payload = lanes.mark_payloads.len() as u32;
                    lanes.mark_payloads.push(payload);
                }
                let source = source(&mut lanes.marks, marks, placed, index);
                let planned =
                    Planned::new(LaneKind::Marks, source, placed.ink).keyed([marks.page, 0]);
                if union { planned.united() } else { planned }
            }
        };
        self.pend(planned, None, placed.orders.of(lane, index));
    }

    /// Returns the ink of one of several instances of an item, cut to the item's ink.
    fn own_ink(&self, bounds: LocalRect, space: u32, placed: &Placed<'_>) -> Option<DeviceRect> {
        if placed.alone {
            return Some(placed.ink);
        }

        let world = self.tables.spaces.get(space)?;
        map(world, bounds).pixels(placed.ink)
    }
}

/// Returns the bounding box of local rectangles, as `[x, y, width, height]`.
fn union(rects: impl Iterator<Item = LocalRect>) -> LocalRect {
    let mut edges = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];

    for [x, y, width, height] in rects {
        edges = [
            edges[0].min(x),
            edges[1].min(y),
            edges[2].max(x + width),
            edges[3].max(y + height),
        ];
    }

    [edges[0], edges[1], edges[2] - edges[0], edges[3] - edges[1]]
}

/// Returns the run of `values`, the instances from `index` of a lowered array, copying them moved
/// into the lane's items unless they are resident.
fn run<T: Instance>(
    lane: &mut FrameLane<T>,
    values: &[T],
    placed: &Placed<'_>,
    index: u32,
) -> ItemSource {
    let len = values.len() as u32;
    match placed.resident {
        Some(chunk) => ItemSource { chunk, index, len },
        None => {
            let first = lane.items.len() as u32;
            lane.items
                .extend(values.iter().map(|value| value.moved(placed.delta)));
            ItemSource {
                chunk: ItemSource::TRANSIENT,
                index: first,
                len,
            }
        }
    }
}

/// Returns the source of an instance, copying it into the lane's items unless it is resident.
fn source<T: Instance>(
    lane: &mut FrameLane<T>,
    value: T,
    placed: &Placed<'_>,
    index: u32,
) -> ItemSource {
    match placed.resident {
        Some(chunk) => ItemSource::resident(chunk, index),
        None => {
            lane.items.push(value);
            ItemSource::transient(lane.items.len() as u32 - 1)
        }
    }
}
