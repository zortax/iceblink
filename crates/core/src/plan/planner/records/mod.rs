//! What the sweep records about every drawn object, kind by kind.

mod lane;
mod single;

use iceblink::Lanes;

use crate::plan::emission::LaneKind;

pub(crate) use self::lane::{LaneRecords, Placed};
pub(crate) use self::single::{Single, Singles};

/// The drawn objects of a frame, by kind.
#[derive(Debug, Default)]
pub(crate) struct Records {
    lanes: [LaneRecords; 8],
    pub(crate) externals: Singles,
    pub(crate) backdrops: Singles,
    /// Group starts, in increasing order.
    pub(crate) begins: Singles,
    /// Group ends, in increasing order.
    pub(crate) ends: Singles,
}

impl Records {
    pub(crate) fn clear(&mut self) {
        for lane in &mut self.lanes {
            lane.clear();
        }
        self.externals.clear();
        self.backdrops.clear();
        self.begins.clear();
        self.ends.clear();
    }

    pub(crate) fn lane(&self, lane: LaneKind) -> &LaneRecords {
        &self.lanes[lane as usize]
    }

    pub(crate) fn lane_mut(&mut self, lane: LaneKind) -> &mut LaneRecords {
        &mut self.lanes[lane as usize]
    }

    /// Writes the sources of every lane into `lanes`, in draw order.
    pub(crate) fn write_sources(&self, lanes: &mut Lanes) {
        for lane in LaneKind::ALL {
            let sources = match lane {
                LaneKind::Shadows => &mut lanes.shadows.sources,
                LaneKind::Quads => &mut lanes.quads.sources,
                LaneKind::ShaderQuads => &mut lanes.shader_quads.sources,
                LaneKind::Marks => &mut lanes.marks.sources,
                LaneKind::Lines => &mut lanes.lines.sources,
                LaneKind::Mono => &mut lanes.mono.sources,
                LaneKind::Subpixel => &mut lanes.subpixel.sources,
                LaneKind::Color => &mut lanes.color.sources,
            };
            self.lane(lane).write_sources(sources);
        }
    }

    /// Puts every kind into draw order. Group markers are pushed in increasing order already.
    pub(crate) fn sort(&mut self) {
        for lane in &mut self.lanes {
            lane.sort();
        }
        self.externals.sort();
        self.backdrops.sort();
    }
}
