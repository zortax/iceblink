//! The planner: one frame's emission stream in, the frame's plan out.

mod cull;
mod inspect;
mod io;
pub(crate) mod records;
mod stats;
mod sweep;

use iceblink::damage::DamageSet;
use iceblink::{Batch, DeviceRect, MarkPlan, OpaqueCover};

use crate::plan::batch::merge::{self, Sequences};
use crate::plan::emission::LaneKind;
use crate::plan::marks::{self, UnionMark};
use crate::plan::order::BoundsTree;
use crate::plan::pass::ClipChains;
use crate::plan::pass::coalesce::Coalescer;

pub use self::io::{Input, Output};
pub use self::stats::PlanStats;

use self::records::Records;

/// Plans frames from their emission streams.
///
/// A planner keeps its scratch space and reuses its allocations from frame to frame.
///
/// # What a plan holds
///
/// [`Planner::plan`] writes these parts of a frame:
///
/// - the `sources` of every lane, in draw order
/// - `batches`, the draw sequence
/// - `vector_plan`, the vector passes
/// - `mark_plan`, the bins of union marks
/// - `opaque_covers`, at most four, from quads drawn before every group in a frame without
///   backdrops
///
/// The plan draws every instance, path and external quad whose ink meets the draw damage on the
/// output, and leaves out the others. Inside groups with filters, the damage is grown by the
/// filters' reach, past the output too. Backdrops and groups are always drawn. No two
/// overlapping instances share a draw order.
///
/// ```
/// use iceblink::damage::DamageSet;
/// use iceblink_core::plan::{ClipChains, Emission, Input, Instance, LaneKind, Output, Planner};
/// use iceblink::geom::{Point, Size};
/// use iceblink::{DeviceRect, Frame, ItemSource};
///
/// struct NoClips;
///
/// impl ClipChains for NoClips {
///     fn parent(&self, _: u32) -> u32 { u32::MAX }
///     fn has_mask(&self, _: u32) -> bool { false }
///     fn residual(&mut self, clip: u32, _: u32) -> u32 { clip }
/// }
///
/// let rect = |x: i32| DeviceRect::new(Point::new(x, 0), Size::new(10, 10));
/// let mut emission = Emission::new();
/// emission.push(Instance::new(LaneKind::Quads, ItemSource::transient(0), rect(0)));
/// emission.push(Instance::new(LaneKind::Quads, ItemSource::transient(1), rect(20)));
///
/// let mut frame = Frame::default();
/// let mut planner = Planner::new();
/// let damage = DamageSet::full();
/// let input = Input::new(&emission, &damage, [64, 64]);
/// let stats = planner.plan(&input, &mut NoClips, Output::of(&mut frame));
///
/// assert_eq!(frame.lanes.quads.drawn(), 2);
/// assert_eq!(frame.batches.len(), 1);
/// assert_eq!(stats.drawn, 2);
/// ```
#[derive(Debug, Default)]
pub struct Planner {
    tree: BoundsTree,
    records: Records,
    coalescer: Coalescer,
    unions: Vec<UnionMark>,
    positions: Vec<u32>,
    /// The members of a block that are drawn, as offsets into the block.
    survivors: Vec<u32>,
    /// The open groups, innermost last, with their bounds.
    groups: Vec<(u32, DeviceRect)>,
    size: [u32; 2],
    wide_backdrop: bool,
}

impl Planner {
    /// Creates a planner with empty scratch space.
    pub fn new() -> Self {
        Self::default()
    }

    /// Plans the frame `input` describes into `output`, and returns its counts.
    ///
    /// `clips` reads the frame clip table and mints the residual chains the vector plan needs.
    /// The plan leaves every lane's `items` and the frame's paths, external quads, backdrops and
    /// groups unchanged.
    pub fn plan(
        &mut self,
        input: &Input<'_>,
        clips: &mut impl ClipChains,
        output: Output<'_>,
    ) -> PlanStats {
        self.records.clear();
        self.coalescer.clear();
        self.unions.clear();
        self.groups.clear();
        self.tree.clear();
        self.size = input.size;
        self.wide_backdrop = input.emission.wide_backdrops > 0;
        output.vector_plan.clear();

        let mut stats = PlanStats::default();
        self.sweep(input, clips, output.vector_plan, &mut stats);
        self.records.sort();

        self.records.write_sources(output.lanes);

        self.replan_marks(input.damage, output.mark_plan);

        merge::merge(
            &Sequences {
                records: &self.records,
                passes: &output.vector_plan.passes,
                pass_orders: self.coalescer.orders(),
                covers: &input.emission.covers,
                size: input.size,
                // A backdrop reads what lies beneath it, so nothing beneath it may be skipped.
                covers_allowed: input.emission.backdrops == 0,
            },
            output.batches,
            output.opaque_covers,
        );

        self.count_instances(output.batches, output.opaque_covers);

        stats.order_inserts = self.tree.inserts();
        stats.batches = output.batches.len() as u32;
        stats.vector_passes = output.vector_plan.passes.len() as u32;
        stats.clip_layers = self.coalescer.clip_layers();
        stats.mark_bins = (output.mark_plan.bins.len() + output.mark_plan.overflow.len()) as u32;
        stats
    }

    /// Turns the batch ranges and cover positions of lanes whose records hold several instances
    /// from draw positions into instance positions.
    fn count_instances(&mut self, batches: &mut [Batch], covers: &mut [OpaqueCover]) {
        for lane in LaneKind::ALL {
            let records = self.records.lane(lane);
            if !records.has_runs() {
                continue;
            }

            records.starts(&mut self.positions);
            let starts = &self.positions;
            for batch in batches.iter_mut() {
                if merge::lane_of_batch(batch.kind) == Some(lane) {
                    let range = &mut batch.range;
                    *range = starts[range.start as usize]..starts[range.end as usize];
                }
            }
            if lane == LaneKind::Quads {
                for cover in covers.iter_mut() {
                    cover.at = starts[cover.at as usize];
                }
            }
        }
    }

    /// Plans the bins of the last planned frame's union marks against `damage` into `plan`.
    ///
    /// The marks and their positions in the marks lane are the last plan's. This is the mark plan
    /// of a redraw of `damage` with the same instances, such as a redraw of the whole output.
    pub fn replan_marks(&mut self, damage: &DamageSet, plan: &mut MarkPlan) {
        if !self.unions.is_empty() {
            self.records
                .lane(LaneKind::Marks)
                .positions(&mut self.positions);
        }
        marks::plan(
            &self.unions,
            &self.positions,
            marks::Frame {
                size: self.size,
                damage,
                wide_backdrop: self.wide_backdrop,
            },
            plan,
        );
    }
}
