//! Planning a walked frame: order, batches, opaque covers, vector passes and mark bins.

mod chains;

use iceblink::Frame;

use crate::frame::walk::Scratch;
use crate::plan::{Input, Output, PlanStats, Planner};
use crate::tables::FrameTables;

use self::chains::Chains;

/// Plans the frame the walk emitted into `scratch`, with passes uncut to the damage if `uncut`,
/// and returns what planning counted.
pub(crate) fn plan(
    planner: &mut Planner,
    scratch: &Scratch,
    tables: &mut FrameTables,
    frame: &mut Frame,
    serial: u64,
    uncut: bool,
) -> PlanStats {
    tables.chains.begin();
    for &clip in &scratch.path_clips {
        tables.chains.note(&mut tables.clips, clip, serial);
    }

    let mut chains = Chains {
        clips: &mut tables.clips,
        intern: &mut tables.chains,
        serial,
    };
    let mut input = Input::new(&scratch.emission, &frame.damage.draw, frame.size);
    input.uncut = uncut;
    let output = Output {
        lanes: &mut frame.lanes,
        batches: &mut frame.batches,
        vector_plan: &mut frame.vector_plan,
        mark_plan: &mut frame.mark_plan,
        opaque_covers: &mut frame.opaque_covers,
    };

    let stats = planner.plan(&input, &mut chains, output);
    tables.chains.trim(&mut tables.clips, serial);
    stats
}
