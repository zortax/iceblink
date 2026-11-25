//! The instances a frame draws, resolved from its lanes into one array per kind in lane order.

use bytemuck::Zeroable as _;
use iceblink::{
    ColorSprite, Frame, FrameMarkPayload, FrameMarks, FrameShadow, Instance, Lane, Line,
    LoweredChunk, Quad, Sprite,
};

use crate::draw::marks::MarkInstance;
use crate::frame::resident::Resident;

/// Every instance of a frame, at its final position, indexed by its position in its lane.
#[derive(Debug, Default)]
pub(crate) struct Instances {
    pub(crate) quads: Vec<Quad>,
    pub(crate) shadows: Vec<FrameShadow>,
    pub(crate) lines: Vec<Line>,
    pub(crate) mono: Vec<Sprite>,
    pub(crate) subpixel: Vec<Sprite>,
    pub(crate) color: Vec<ColorSprite>,
    pub(crate) marks: Vec<MarkInstance>,
}

impl Instances {
    /// Resolves the lanes of `frame` against the resident chunks.
    ///
    /// A run naming a chunk the renderer does not hold, or instances past a chunk's array,
    /// resolves to empty instances, which draw nothing and keep later positions in place.
    pub(crate) fn resolve(&mut self, frame: &Frame, resident: &Resident) {
        let lanes = &frame.lanes;
        resolve(&mut self.quads, &lanes.quads, frame, resident, |chunk| {
            &chunk.quads
        });
        resolve(
            &mut self.shadows,
            &lanes.shadows,
            frame,
            resident,
            |chunk| &chunk.shadows,
        );
        resolve(&mut self.lines, &lanes.lines, frame, resident, |chunk| {
            &chunk.lines
        });
        resolve(&mut self.mono, &lanes.mono, frame, resident, |chunk| {
            &chunk.mono
        });
        resolve(
            &mut self.subpixel,
            &lanes.subpixel,
            frame,
            resident,
            |chunk| &chunk.subpixel,
        );
        resolve(&mut self.color, &lanes.color, frame, resident, |chunk| {
            &chunk.color
        });
        self.marks.clear();
        for run in &lanes.marks.sources {
            for index in run.indices() {
                self.marks.push(mark(frame, resident, run.chunk, index));
            }
        }
    }
}

fn resolve<T: Instance + bytemuck::Zeroable>(
    into: &mut Vec<T>,
    lane: &Lane<T>,
    frame: &Frame,
    resident: &Resident,
    array: impl Fn(&LoweredChunk) -> &Vec<T>,
) {
    into.clear();
    for run in &lane.sources {
        let range = run.indices();
        if run.is_transient() {
            into.extend(range.map(|index| {
                lane.items
                    .get(index as usize)
                    .copied()
                    .unwrap_or_else(T::zeroed)
            }));
            continue;
        }
        let drawn = frame.drawn.get(run.chunk as usize);
        let chunk = drawn.and_then(|drawn| resident.get(drawn.revision));
        let offset = drawn.map_or([0.0; 2], |drawn| drawn.offset);
        into.extend(range.map(|index| {
            chunk
                .and_then(|chunk| array(chunk).get(index as usize))
                .map_or_else(T::zeroed, |instance| instance.moved(offset))
        }));
    }
}

fn mark(frame: &Frame, resident: &Resident, chunk: u32, index: u32) -> MarkInstance {
    let empty = || MarkInstance {
        item: FrameMarks::zeroed(),
        payload: None,
    };
    let payload = |payloads: &[FrameMarkPayload], item: &FrameMarks| {
        payloads.get(item.marks.payload as usize).cloned()
    };
    if chunk == iceblink::ItemSource::TRANSIENT {
        let Some(item) = frame.lanes.marks.items.get(index as usize) else {
            return empty();
        };
        return MarkInstance {
            item: *item,
            payload: payload(&frame.lanes.mark_payloads, item),
        };
    }
    let Some(drawn) = frame.drawn.get(chunk as usize) else {
        return empty();
    };
    let Some(lowered) = resident.get(drawn.revision) else {
        return empty();
    };
    let Some(item) = lowered.marks.get(index as usize) else {
        return empty();
    };
    MarkInstance {
        item: item.moved(drawn.offset),
        payload: payload(&lowered.mark_payloads, item),
    }
}
