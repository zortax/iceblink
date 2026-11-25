//! The compositor's scroll state of one scroll node.

use iceblink::{ScrollBehaviour, ScrollId, ScrollNode, ScrollPhase, ScrollSource};

use crate::geometry::bounds::axis_scale;
use crate::scene::record::ScrollRecord;
use crate::tier::View;
use crate::tier::scroll::motion::Motion;
use crate::tier::scroll::serials::Serials;

/// A bounce return under way on one axis, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Return {
    pub(crate) from: f64,
    /// The velocity at the start, in device pixels per second.
    pub(crate) speed: f64,
    pub(crate) start_ns: u64,
}

/// The compositor's motion of one axis of a scroll node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Axis {
    pub(crate) motion: Motion,
    /// The pull past the end that a finger holds, in device pixels.
    pub(crate) pull: f64,
    pub(crate) ret: Option<Return>,
    /// The overscroll displacement, in device pixels.
    pub(crate) displacement: f64,
}

impl Axis {
    /// Reports whether the axis moves on its own or is held past its end.
    pub(crate) fn is_active(&self) -> bool {
        self.motion != Motion::Idle || self.ret.is_some() || self.pull != 0.0
    }
}

/// The compositor's scroll state of one scroll node: the client's base offset, the compositor
/// delta that input and physics added, and the motion under way.
#[derive(Debug)]
pub(crate) struct ScrollState {
    pub(crate) id: ScrollId,
    /// The offset the client last wrote, clamped to the range when written.
    pub(crate) base: [f32; 2],
    /// The compositor delta, in the node's units.
    pub(crate) delta: [f64; 2],
    /// The value of an animation of the node's offset, which stands in for the base.
    pub(crate) programmatic: Option<[f32; 2]>,
    /// Whether an animation of the offset has not finished.
    pub(crate) animated: bool,
    pub(crate) axes: [Axis; 2],
    pub(crate) serials: Serials,
    /// What moved the node last.
    pub(crate) cause: ScrollPhase,
    /// The source of the gesture that moved the node last.
    pub(crate) source: ScrollSource,
    /// Whether the node moved since it last came to rest.
    pub(crate) unsettled: bool,
    /// The offset and overscroll the last event carried.
    pub(crate) shown: ([f32; 2], [f32; 2]),
}

impl ScrollState {
    /// Returns the state of a node the compositor has not moved, at the offset `base`.
    pub(crate) fn new(id: ScrollId, base: [f32; 2]) -> Self {
        Self {
            id,
            base,
            delta: [0.0; 2],
            programmatic: None,
            animated: false,
            axes: [Axis::default(); 2],
            serials: Serials::default(),
            cause: ScrollPhase::Gesture,
            source: ScrollSource::Wheel,
            unsettled: false,
            shown: (base, [0.0; 2]),
        }
    }

    /// Returns the offset the delta is added to: the base, or the value of an animation of the
    /// offset.
    pub(crate) fn origin(&self) -> [f32; 2] {
        self.programmatic.unwrap_or(self.base)
    }

    /// Returns the exact current offset on `axis`, clamped to `[0, max]`.
    pub(crate) fn exact(&self, axis: usize, max: f32) -> f64 {
        (f64::from(self.origin()[axis]) + self.delta[axis]).clamp(0.0, f64::from(max))
    }

    /// Reports whether the node moves on its own or is held past an end.
    pub(crate) fn is_active(&self) -> bool {
        self.animated || self.axes.iter().any(Axis::is_active)
    }

    /// Reports whether the node moves on its own.
    pub(crate) fn moves(&self) -> bool {
        self.animated
            || self
                .axes
                .iter()
                .any(|axis| axis.motion != Motion::Idle || axis.ret.is_some())
    }

    /// Ends the offset animation's stand-in for the base, moving its difference from the base
    /// into the delta so the offset stays where it is, and reports whether there was one.
    pub(crate) fn fold_programmatic(&mut self) -> bool {
        let Some(origin) = self.programmatic.take() else {
            return false;
        };
        for ((delta, held), (origin, base)) in self
            .delta
            .iter_mut()
            .zip(&mut self.axes)
            .zip(origin.iter().zip(&self.base))
        {
            let by = f64::from(origin - base);
            *delta += by;
            held.motion.shift(by);
        }
        true
    }

    /// Ends every motion, keeping the delta where it is at `now_ns`.
    pub(crate) fn stop(&mut self, now_ns: u64) {
        for (held, delta) in self.axes.iter_mut().zip(&mut self.delta) {
            if let Some(step) = held.motion.sample(now_ns) {
                *delta = step.delta;
            }
            held.motion = Motion::Idle;
        }
    }
}

/// A scroll node's geometry on the device, for a node whose space scales the plane by a
/// positive factor on each axis.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    /// The largest offset, per axis.
    pub(crate) max: [f32; 2],
    /// Device pixels per unit, per axis.
    pub(crate) scale: [f64; 2],
    /// The port on the device, as `[x0, y0, x1, y1]`.
    pub(crate) port: [f32; 4],
    /// The port's extent per axis, in units.
    pub(crate) extent: [f32; 2],
}

impl Geometry {
    /// Returns the geometry of `record`, or `None` when its space does not scale the plane by a
    /// positive factor on each axis.
    pub(crate) fn of(view: &View<'_>, record: &ScrollRecord) -> Option<Self> {
        let world = view.transforms.at(record.node.space.slot())?.world;
        let [sx, sy, tx, ty] = axis_scale(&world)?;
        if !(sx > 0.0 && sy > 0.0) {
            return None;
        }

        let [x, y, width, height] = record.node.port;
        Some(Self {
            max: range(&record.node),
            scale: [f64::from(sx), f64::from(sy)],
            port: [
                x * sx + tx,
                y * sy + ty,
                (x + width) * sx + tx,
                (y + height) * sy + ty,
            ],
            extent: [width, height],
        })
    }

    /// Returns the device pixels per unit of `node` per axis, or `None` when its space does not
    /// scale the plane by a positive factor on each axis.
    pub(crate) fn scale_of(view: &View<'_>, node: &ScrollNode) -> Option<[f64; 2]> {
        let world = view.transforms.at(node.space.slot())?.world;
        let [sx, sy, ..] = axis_scale(&world)?;
        (sx > 0.0 && sy > 0.0).then(|| [f64::from(sx), f64::from(sy)])
    }

    /// Reports whether the port holds the device point `position`, edges included.
    pub(crate) fn holds(&self, [x, y]: [f32; 2]) -> bool {
        let port = self.port;
        port[0] <= x && x <= port[2] && port[1] <= y && y <= port[3]
    }

    /// Returns the port's extent on `axis` in device pixels.
    pub(crate) fn device_extent(&self, axis: usize) -> f64 {
        f64::from(self.extent[axis]) * self.scale[axis]
    }
}

/// Returns the largest offset of a scroll node, per axis.
pub(crate) fn range(node: &ScrollNode) -> [f32; 2] {
    [
        (node.content[0] - node.port[2]).max(0.0),
        (node.content[1] - node.port[3]).max(0.0),
    ]
}

/// Returns the offset and the overscroll displacement a node shows, in its units.
///
/// The compositor delta and the displacement are rounded to whole device pixels at `scale`, so
/// input and physics move content by whole pixels. Without a scale they are not applied.
pub(crate) fn applied(
    state: &ScrollState,
    max: [f32; 2],
    scale: Option<[f64; 2]>,
) -> ([f32; 2], [f32; 2]) {
    let offset = shown_offset(state.origin(), state.delta, max, scale);
    let overscroll = [0, 1].map(|axis| match scale {
        Some(scale) => (state.axes[axis].displacement.round() / scale[axis]) as f32,
        None => 0.0,
    });
    (offset, overscroll)
}

/// Returns the offset a node with origin `origin` and compositor delta `delta` shows: their sum,
/// with the delta rounded to whole device pixels at `scale`, clamped to `[0, max]`.
pub(crate) fn shown_offset(
    origin: [f32; 2],
    delta: [f64; 2],
    max: [f32; 2],
    scale: Option<[f64; 2]>,
) -> [f32; 2] {
    let delta = applied_delta(delta, scale);
    [0, 1]
        .map(|axis| (f64::from(origin[axis]) + delta[axis]).clamp(0.0, f64::from(max[axis])) as f32)
}

/// Returns a compositor delta rounded to the nearest whole device pixel at `scale`, or zero
/// without one.
///
/// A delta half a pixel past a whole one rounds up, so the rest of a delta after the rounded delta
/// is taken out of it rounds to zero.
pub(crate) fn applied_delta(delta: [f64; 2], scale: Option<[f64; 2]>) -> [f64; 2] {
    match scale {
        Some(scale) => [0, 1].map(|axis| whole(delta[axis] * scale[axis]) / scale[axis]),
        None => [0.0; 2],
    }
}

/// Returns `pixels` rounded to the nearest whole number, halves up.
fn whole(pixels: f64) -> f64 {
    // A rest left after a half rounded up lies a hair below minus a half once it went through
    // node units, so the slack keeps it from rounding down a pixel.
    const SLACK: f64 = 1e-6;
    (pixels + 0.5 + SLACK).floor()
}

/// Returns the behaviour of a scroll node, if the compositor scrolls it.
pub(crate) fn behaviour(record: &ScrollRecord) -> Option<&ScrollBehaviour> {
    record.node.behaviour.as_ref()
}
