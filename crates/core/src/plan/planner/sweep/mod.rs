//! The sweep over the emission stream: culling, ordering and vector coalescing.

mod block;
mod group;
mod instance;
mod single;

use iceblink::VectorPlan;

use crate::plan::emission::Entry;
use crate::plan::pass::ClipChains;
use crate::plan::pass::coalesce::{self, OpenPath};
use crate::plan::planner::cull::Cull;
use crate::plan::planner::{Input, PlanStats, Planner};

/// What every step of one sweep reads.
#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    pub(super) input: &'a Input<'a>,
    pub(super) cull: Cull<'a>,
    pub(super) pass: coalesce::Frame,
}

impl Planner {
    /// Sweeps the stream in paint order: culls, assigns draw orders, records every drawn object by
    /// kind and feeds the vector passes into `vector_plan`.
    pub(super) fn sweep(
        &mut self,
        input: &Input<'_>,
        clips: &mut impl ClipChains,
        vector_plan: &mut VectorPlan,
        stats: &mut PlanStats,
    ) {
        let emission = input.emission;
        let damage = input.damage;
        let mut context = Context {
            input,
            cull: Cull::new(input.size, damage),
            pass: coalesce::Frame {
                size: input.size,
                // A backdrop keeps every pass whole.
                cut: if damage.is_full() || emission.backdrops > 0 || input.uncut {
                    None
                } else {
                    damage.bounds()
                },
            },
        };

        let entries = &emission.entries;
        let mut reaches: Vec<i32> = Vec::new();
        let mut at = 0;
        while at < entries.len() {
            match entries[at] {
                Entry::Instance { instance, cover } => {
                    self.instance(instance, cover, context, stats);
                }
                Entry::Block { len } => {
                    let members = &entries[at + 1..(at + 1 + len as usize).min(entries.len())];
                    self.block(members, context, stats);
                    at += members.len();
                }
                Entry::Path {
                    index,
                    ink,
                    reach,
                    clip,
                } => {
                    if context.cull.drawn(ink) {
                        self.coalescer.prepare(
                            clip,
                            ink,
                            input.overlap,
                            context.pass,
                            clips,
                            vector_plan,
                        );
                        self.raise();
                        let order = self.tree.insert(ink);
                        self.coalescer.admit(
                            OpenPath {
                                index,
                                clip,
                                order,
                                reach,
                            },
                            ink,
                        );
                        stats.drawn += 1;
                    } else {
                        stats.culled += 1;
                        stats.paths_culled += 1;
                    }
                }
                Entry::External { index, ink } => self.external(index, ink, context, stats),
                Entry::Backdrop { index, ink } => self.backdrop(index, ink, context, stats),
                Entry::GroupBegin {
                    index,
                    bounds,
                    reach,
                } => {
                    self.begin_group(index, bounds, context, clips, vector_plan);
                    reaches.push(context.cull.reach);
                    context.cull.reach = context.cull.reach.saturating_add(reach);
                    stats.groups += 1;
                }
                Entry::GroupEnd => {
                    self.end_group(context, clips, vector_plan);
                    context.cull.reach = reaches.pop().unwrap_or(0);
                }
            }
            at += 1;
        }

        debug_assert!(self.groups.is_empty(), "every group begun is ended");
        while !self.groups.is_empty() {
            self.end_group(context, clips, vector_plan);
        }
        self.coalescer.finish(context.pass, clips, vector_plan);
    }

    /// Raises the bounds tree over the paths of the passes closed since the last call, so later
    /// overlapping instances sort above their composites.
    pub(super) fn raise(&mut self) {
        let tree = &mut self.tree;
        self.coalescer
            .drain_raises(|ink, order| tree.raise(ink, order));
    }
}
