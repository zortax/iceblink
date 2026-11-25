//! A frame's header: its id, output, clocks, shifts and damage.

use iceblink::damage::DamageSet;
use iceblink::{Frame, FrameId, FrameRequest, OutputId, ShaderClock};

use crate::frame::age;
use crate::output::Output;
use crate::shift::Decision;

/// Empties `frame` and fills its header for the next frame of `output`, with a shader clock
/// that counts from `clock_start`.
pub(crate) fn fill(
    frame: &mut Frame,
    (id, output): (OutputId, &mut Output),
    request: &FrameRequest<'_>,
    clock_start: u64,
    decision: &Decision,
    own: DamageSet,
) {
    frame.clear();

    frame.id = FrameId(output.next_frame);
    output.next_frame += 1;
    frame.output = id;
    frame.size = output.config.size;
    frame.scale = output.config.scale;
    frame.opaque = output.config.opaque;
    frame.clock = request.clock;
    frame.shader_clock = shader_clock(
        request.clock.present_ns,
        clock_start,
        output.last_present_ns,
    );
    output.last_present_ns = Some(request.clock.present_ns);
    frame.shifts = decision.shifts.clone();
    frame.damage = age::split(own, &decision.shifts, request.age, &output.history);
    frame.damage.driven = output.driven;
}

/// Returns the shader clock of a frame presented at `present_ns`, counting from `start`, after a
/// frame presented at `last`.
fn shader_clock(present_ns: u64, start: u64, last: Option<u64>) -> ShaderClock {
    const SECOND: f64 = 1e9;
    ShaderClock {
        time: (present_ns.saturating_sub(start) as f64 / SECOND) as f32,
        delta: last.map_or(0.0, |last| {
            (present_ns.saturating_sub(last) as f64 / SECOND) as f32
        }),
    }
}
