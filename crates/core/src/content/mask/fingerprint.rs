//! The exact geometry a path mask is kept by.

use core::hash::{Hash, Hasher};

use kurbo::{Affine, BezPath, PathEl};
use rustc_hash::FxHasher;

use crate::content::coverage::Style;
use crate::content::mask::{MaskRequest, Part};

/// One path command, as the `f32` bits of its points in mask texels from the mask's corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Command {
    Move([u32; 2]),
    Line([u32; 2]),
    Quad([u32; 4]),
    Curve([u32; 6]),
    Close,
}

/// A mask's geometry, independent of whole-texel moves and of its paint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Fingerprint {
    pub(super) commands: Box<[Command]>,
    pub(super) style: Style,
    pub(super) size: [u32; 2],
}

impl Fingerprint {
    /// Returns the fingerprint of `request`, or `None` for a stroke with no width or a point or
    /// length that is not finite.
    pub(super) fn of(request: &MaskRequest<'_>) -> Option<Self> {
        let [_, _, width, height] = request.bounds;
        let style = match request.part {
            Part::Fill(even_odd) => Style::Fill(even_odd),
            Part::Stroke(stroke) => Style::stroke(stroke, request.density[0])?,
        };
        let commands = commands(
            request.path,
            request.placement,
            request.density,
            request.phase,
            [request.bounds[0], request.bounds[1]],
        )?;
        Some(Self {
            commands,
            style,
            size: [width as u32, height as u32],
        })
    }

    /// Returns a digest of the fingerprint, which route histories compare masks by.
    pub(super) fn digest(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns the number of texels the mask covers.
    pub(super) fn texels(&self) -> u64 {
        u64::from(self.size[0]) * u64::from(self.size[1])
    }
}

/// Returns the commands of `path` in mask texels: placed by `placement`, scaled by `density`,
/// moved by `phase` and then by `-corner`.
///
/// Returns `None` when a point is not finite.
fn commands(
    path: &BezPath,
    placement: Affine,
    density: [f32; 2],
    phase: [f32; 2],
    corner: [i32; 2],
) -> Option<Box<[Command]>> {
    let kx = f64::from(density[0]);
    let ky = f64::from(density[1]);
    let x = f64::from(corner[0]) - f64::from(phase[0]);
    let y = f64::from(corner[1]) - f64::from(phase[1]);
    let point = |point: kurbo::Point| {
        let point = placement * point;
        let mapped = [(point.x * kx - x) as f32, (point.y * ky - y) as f32];
        (mapped[0].is_finite() && mapped[1].is_finite())
            .then_some([mapped[0].to_bits(), mapped[1].to_bits()])
    };

    path.elements()
        .iter()
        .map(|element| match *element {
            PathEl::MoveTo(p) => Some(Command::Move(point(p)?)),
            PathEl::LineTo(p) => Some(Command::Line(point(p)?)),
            PathEl::QuadTo(a, b) => {
                let [a, b] = [point(a)?, point(b)?];
                Some(Command::Quad([a[0], a[1], b[0], b[1]]))
            }
            PathEl::CurveTo(a, b, c) => {
                let [a, b, c] = [point(a)?, point(b)?, point(c)?];
                Some(Command::Curve([a[0], a[1], b[0], b[1], c[0], c[1]]))
            }
            PathEl::ClosePath => Some(Command::Close),
        })
        .collect()
}

/// Returns the coverage of `fingerprint`, one byte per texel.
pub(super) fn raster(fingerprint: &Fingerprint) -> Vec<u8> {
    let point = |bits: [u32; 2]| zeno::Point::new(f32::from_bits(bits[0]), f32::from_bits(bits[1]));
    let commands: Vec<zeno::Command> = fingerprint
        .commands
        .iter()
        .map(|command| match *command {
            Command::Move(p) => zeno::Command::MoveTo(point(p)),
            Command::Line(p) => zeno::Command::LineTo(point(p)),
            Command::Quad(p) => zeno::Command::QuadTo(point([p[0], p[1]]), point([p[2], p[3]])),
            Command::Curve(p) => zeno::Command::CurveTo(
                point([p[0], p[1]]),
                point([p[2], p[3]]),
                point([p[4], p[5]]),
            ),
            Command::Close => zeno::Command::Close,
        })
        .collect();

    crate::content::coverage::raster(&commands, &fingerprint.style, fingerprint.size)
}
