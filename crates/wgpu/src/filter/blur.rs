//! The blur chain: a half-resolution downsample, then a horizontal and a vertical pass.

use iceblink::geom::{Device, Edges, Rect};

use crate::filter::EdgeMode;
use crate::frame::build::PlanBuilder;
use crate::frame::segment::PlannedDraw;
use crate::frame::target::TargetRef;
use crate::pipeline::blur::{BlurAxis, BlurParams};
use crate::target::scale::TargetScale;

/// A planned blur chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Blurred {
    /// The half-resolution target that holds the blurred content.
    pub(crate) target: TargetRef,
}

/// How many standard deviations a blur reads.
// Three deviations is where a gaussian's tail falls below half a level at eight bits.
const BLUR_EXTENT: f32 = 3.0;

/// Returns how far a blur of `deviation` reads past the pixel it writes.
pub(crate) fn reach(deviation: f32) -> i32 {
    (BLUR_EXTENT * deviation.max(0.0)).ceil() as i32
}

/// Plans a blur of `source` with the standard deviation `deviation`, written over `output`.
///
/// Reads outside `valid`, the device pixels `source` holds, are treated as `edge` says.
///
/// Returns `None` when the two half-resolution targets cannot be acquired.
pub(crate) fn plan(
    builder: &mut PlanBuilder<'_>,
    source: TargetRef,
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
    deviation: f32,
    edge: EdgeMode,
) -> Option<Blurred> {
    debug_assert!(
        valid.contains_rect(output),
        "a blur writing {output:?} reads outside the {valid:?} its source holds"
    );
    let reach = reach(deviation);
    let downsampled = grown(output, reach, reach, valid);
    let horizontal = grown(output, 0, reach, valid);
    let ping = builder.acquire(TargetScale::Half, valid)?;
    let pong = match builder.acquire(TargetScale::Half, valid) {
        Some(pong) => pong,
        None => {
            builder.release(ping);
            return None;
        }
    };
    let ping = TargetRef::Pool(ping);
    let pong = TargetRef::Pool(pong);

    // Each pass writes a texel past its region for the next, cut to what the source holds.
    let promised = |region| grown(region, MARGIN, MARGIN, valid);

    let downsample = builder.stage_blur(&BlurParams::downsample(
        (builder.extent_of(source), source.origin()),
        source.scale(),
        (builder.extent_of(ping), ping.origin()),
        valid,
        edge,
    ));
    // Pass regions are in device pixels. Opening a pass into a half-resolution target converts
    // its scissor.
    builder.begin_pass(ping, with_slack(downsampled, valid));
    builder.draw(PlannedDraw::Blur {
        source,
        params: downsample,
        downsample: true,
    });

    // Each axis is told what the pass before it wrote, which is what its reads are clamped to.
    for (axis, from, to, region, written) in [
        (
            BlurAxis::Horizontal,
            ping,
            pong,
            horizontal,
            promised(downsampled),
        ),
        (BlurAxis::Vertical, pong, ping, output, promised(horizontal)),
    ] {
        let params = builder.stage_blur(&BlurParams::axis(
            [builder.extent_of(from), builder.extent_of(to)],
            TargetScale::Half,
            [from.origin(), to.origin()],
            axis,
            deviation,
            written,
            edge,
        ));
        builder.begin_pass(to, with_slack(region, valid));
        builder.draw(PlannedDraw::Blur {
            source: from,
            params,
            downsample: false,
        });
    }

    if let Some(slot) = pong.slot() {
        builder.release(slot);
    }
    Some(Blurred { target: ping })
}

/// Returns `rect` grown by `x` and `y` on each side, cut to `within`.
fn grown(rect: Rect<i32, Device>, x: i32, y: i32, within: Rect<i32, Device>) -> Rect<i32, Device> {
    rect.outset(Edges::axes(x, y))
        .intersection(within)
        .unwrap_or(rect)
}

/// How far past its region, in device pixels, a pass writes for the pass after it.
const MARGIN: i32 = 2;

/// How far past `MARGIN` a pass's scissor reaches, in device pixels.
const SLACK: i32 = 4;

/// Returns the scissor of a pass over `rect`, cut to `within`.
fn with_slack(rect: Rect<i32, Device>, within: Rect<i32, Device>) -> Rect<i32, Device> {
    grown(rect, MARGIN + SLACK, MARGIN + SLACK, within)
}
