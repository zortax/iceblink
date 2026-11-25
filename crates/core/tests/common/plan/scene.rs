//! A frame's emission stream built item by item, with a log of what was pushed.

use iceblink::damage::DamageSet;
use iceblink::{DeviceRect, FRAME_NO_CLIP, Frame, ItemSource};
use iceblink_core::plan::{
    BoundsTree, DrawOrder, Emission, Input, Instance, LaneKind, Output, Overlap, PlanStats,
    Planner, local_orders,
};

use crate::common::plan::clips::ClipTable;

/// What one pushed entry was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pushed {
    /// An instance of a lane, with its transient index and binding key.
    Instance {
        lane: LaneKind,
        index: u32,
        key: [u32; 2],
        union: bool,
        cover: Option<DeviceRect>,
    },
    /// A path, with its frame path index and clip.
    Path {
        index: u32,
        clip: u32,
    },
    External {
        index: u32,
    },
    Backdrop {
        index: u32,
    },
    GroupBegin {
        index: u32,
    },
    GroupEnd {
        index: u32,
    },
}

/// One logged entry and its ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Logged {
    pub(crate) pushed: Pushed,
    pub(crate) ink: DeviceRect,
}

/// An emission stream with the clip table of its paths, and a log of every entry pushed.
#[derive(Debug)]
pub(crate) struct Scene {
    pub(crate) emission: Emission,
    pub(crate) clips: ClipTable,
    pub(crate) size: [u32; 2],
    pub(crate) log: Vec<Logged>,
    counts: [u32; 8],
    open: Vec<(u32, DeviceRect)>,
}

/// A planned frame, with the planner that planned it.
#[derive(Debug)]
pub(crate) struct Planned {
    pub(crate) frame: Frame,
    pub(crate) stats: PlanStats,
    pub(crate) planner: Planner,
}

impl Scene {
    /// Returns an empty scene over a 1920 by 1080 output.
    pub(crate) fn new() -> Self {
        Self::sized([1920, 1080])
    }

    /// Returns an empty scene over an output of `size`.
    pub(crate) fn sized(size: [u32; 2]) -> Self {
        Self {
            emission: Emission::new(),
            clips: ClipTable::default(),
            size,
            log: Vec::new(),
            counts: [0; 8],
            open: Vec::new(),
        }
    }

    /// Returns the next transient instance of `lane` over `ink`.
    pub(crate) fn next(&mut self, lane: LaneKind, ink: DeviceRect) -> Instance {
        let index = self.counts[lane as usize];
        self.counts[lane as usize] += 1;
        Instance::new(lane, ItemSource::transient(index), ink)
    }

    /// Pushes `instance`.
    pub(crate) fn push(&mut self, instance: Instance) {
        self.log_instance(instance, None);
        self.emission.push(instance);
    }

    /// Pushes a quad over `ink`.
    pub(crate) fn quad(&mut self, ink: DeviceRect) {
        let instance = self.next(LaneKind::Quads, ink);
        self.push(instance);
    }

    /// Pushes an instance of `lane` over `ink` with `key`.
    pub(crate) fn keyed(&mut self, lane: LaneKind, ink: DeviceRect, key: [u32; 2]) {
        let instance = self.next(lane, ink).keyed(key);
        self.push(instance);
    }

    /// Pushes a quad over `ink` that covers `cover`.
    pub(crate) fn cover(&mut self, ink: DeviceRect, cover: DeviceRect) {
        let instance = self.next(LaneKind::Quads, ink);
        self.log_instance(instance, Some(cover));
        self.emission.push_cover(instance, cover);
    }

    /// Pushes a block of instances of `lane` over `inks`, ordered among themselves.
    pub(crate) fn block(&mut self, lane: LaneKind, inks: &[DeviceRect]) {
        let mut tree = BoundsTree::new();
        let mut locals = Vec::new();
        local_orders(inks, &mut tree, &mut locals);
        self.block_with(lane, inks, &locals);
    }

    /// Pushes a block of instances of `lane` over `inks` with the local orders `locals`.
    pub(crate) fn block_with(&mut self, lane: LaneKind, inks: &[DeviceRect], locals: &[DrawOrder]) {
        self.emission.begin_block();
        for (ink, local) in inks.iter().zip(locals) {
            let instance = self.next(lane, *ink).at_local(*local);
            self.push(instance);
        }
        self.emission.end_block();
    }

    /// Pushes a mark over `ink`, flagged as a union mark if `union`.
    pub(crate) fn mark(&mut self, ink: DeviceRect, union: bool) {
        let mut instance = self.next(LaneKind::Marks, ink);
        instance.union = union;
        self.push(instance);
    }

    /// Pushes a path over `ink` through `clip`, and returns its frame path index.
    pub(crate) fn path(&mut self, ink: DeviceRect, clip: u32) -> u32 {
        self.reaching_path(ink, ink, clip)
    }

    /// Pushes a path over `ink` through `clip` whose outline covers `reach`, and returns its frame
    /// path index.
    pub(crate) fn reaching_path(&mut self, ink: DeviceRect, reach: DeviceRect, clip: u32) -> u32 {
        let index = self.emission.push_path(ink, reach, clip);
        self.log.push(Logged {
            pushed: Pushed::Path { index, clip },
            ink,
        });
        index
    }

    /// Pushes an unclipped path over `ink`, and returns its frame path index.
    pub(crate) fn plain_path(&mut self, ink: DeviceRect) -> u32 {
        self.path(ink, FRAME_NO_CLIP)
    }

    /// Pushes an external quad over `ink`.
    pub(crate) fn external(&mut self, ink: DeviceRect) {
        let index = self.emission.push_external(ink);
        self.log.push(Logged {
            pushed: Pushed::External { index },
            ink,
        });
    }

    /// Pushes a backdrop writing `ink`, whose source covers pixels outside `ink` if
    /// `reads_outside`.
    pub(crate) fn backdrop(&mut self, ink: DeviceRect, reads_outside: bool) {
        let index = self.emission.push_backdrop(ink, reads_outside);
        self.log.push(Logged {
            pushed: Pushed::Backdrop { index },
            ink,
        });
    }

    /// Starts a group writing `bounds`.
    pub(crate) fn begin_group(&mut self, bounds: DeviceRect) {
        let index = self.emission.begin_group(bounds);
        self.open.push((index, bounds));
        self.log.push(Logged {
            pushed: Pushed::GroupBegin { index },
            ink: bounds,
        });
    }

    /// Ends the innermost group.
    pub(crate) fn end_group(&mut self) {
        let (index, bounds) = self.open.pop().expect("a group is open");
        self.emission.end_group();
        self.log.push(Logged {
            pushed: Pushed::GroupEnd { index },
            ink: bounds,
        });
    }

    /// Plans the scene against `damage` with the default reading of the overlap rule.
    pub(crate) fn plan(&mut self, damage: &DamageSet) -> Planned {
        self.plan_with(damage, Overlap::default())
    }

    /// Plans the scene against `damage` with the reading `overlap`.
    pub(crate) fn plan_with(&mut self, damage: &DamageSet, overlap: Overlap) -> Planned {
        let mut frame = Frame::default();
        let mut planner = Planner::new();
        let mut input = Input::new(&self.emission, damage, self.size);
        input.overlap = overlap;

        let stats = planner.plan(&input, &mut self.clips, Output::of(&mut frame));
        Planned {
            frame,
            stats,
            planner,
        }
    }

    fn log_instance(&mut self, instance: Instance, cover: Option<DeviceRect>) {
        self.log.push(Logged {
            pushed: Pushed::Instance {
                lane: instance.lane,
                index: instance.source.index,
                key: instance.key,
                union: instance.union,
                cover,
            },
            ink: instance.ink,
        });
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Planned {
    /// Returns the number of vector passes.
    pub(crate) fn passes(&self) -> usize {
        self.frame.vector_plan.passes.len()
    }

    /// Returns the draw orders of the transient instances of `lane`, by transient index.
    pub(crate) fn orders(&self, lane: LaneKind) -> Vec<Option<u32>> {
        let sources = lane_sources(&self.frame, lane);
        let mut orders = vec![
            None;
            sources
                .iter()
                .map(|source| source.index as usize + 1)
                .max()
                .unwrap_or(0)
        ];
        for (source, order) in sources.iter().zip(self.planner.orders(lane)) {
            orders[source.index as usize] = Some(order);
        }
        orders
    }
}

/// Returns the sources of the instances `lane` draws in `frame`, in draw order, each as a run of
/// one.
pub(crate) fn lane_sources(frame: &Frame, lane: LaneKind) -> Vec<ItemSource> {
    let lanes = &frame.lanes;
    match lane {
        LaneKind::Shadows => lanes.shadows.instances().collect(),
        LaneKind::Quads => lanes.quads.instances().collect(),
        LaneKind::ShaderQuads => lanes.shader_quads.instances().collect(),
        LaneKind::Marks => lanes.marks.instances().collect(),
        LaneKind::Lines => lanes.lines.instances().collect(),
        LaneKind::Mono => lanes.mono.instances().collect(),
        LaneKind::Subpixel => lanes.subpixel.instances().collect(),
        LaneKind::Color => lanes.color.instances().collect(),
    }
}
