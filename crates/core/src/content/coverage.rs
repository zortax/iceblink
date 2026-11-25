//! Path coverage rasterised on the processor, one byte per texel.

use kurbo::{BezPath, Cap, Join, PathEl, Point};

/// How a path's coverage is produced, with every length in texels and held as `f32` bits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Style {
    /// The interior, by the even-odd rule when set and the non-zero rule otherwise.
    Fill(bool),
    /// The outline a stroke produces.
    Stroke {
        width: u32,
        join: u8,
        miter_limit: u32,
        start_cap: u8,
        end_cap: u8,
        dashes: Box<[u32]>,
        dash_offset: u32,
    },
}

impl Style {
    /// Returns the style of `stroke` with every length scaled by `density`, or `None` for a
    /// stroke with no width or a length that is not finite.
    // A stroke does not commute with a map that scales its axes differently, so one density
    // serves both axes and the caller declines the rest.
    pub(crate) fn stroke(stroke: &kurbo::Stroke, density: f32) -> Option<Self> {
        let finite = [stroke.width, stroke.miter_limit, stroke.dash_offset]
            .into_iter()
            .chain(stroke.dash_pattern.iter().copied())
            .all(f64::is_finite);
        if !finite || stroke.width <= 0.0 {
            return None;
        }

        let scaled = |value: f64| ((value * f64::from(density)) as f32).to_bits();
        Some(Self::Stroke {
            width: scaled(stroke.width),
            join: join_code(stroke.join),
            miter_limit: (stroke.miter_limit as f32).to_bits(),
            start_cap: cap_code(stroke.start_cap),
            end_cap: cap_code(stroke.end_cap),
            dashes: stroke
                .dash_pattern
                .iter()
                .map(|value| scaled(*value))
                .collect(),
            dash_offset: scaled(stroke.dash_offset),
        })
    }
}

/// Returns the coverage of `commands` drawn in `style`, `size` texels from the origin, one byte
/// per texel with rows tightly packed.
pub(crate) fn raster(commands: &[zeno::Command], style: &Style, size: [u32; 2]) -> Vec<u8> {
    let mut mask = zeno::Mask::new(commands);
    let dashes: Vec<f32>;
    match style {
        Style::Fill(even_odd) => {
            mask.style(if *even_odd {
                zeno::Fill::EvenOdd
            } else {
                zeno::Fill::NonZero
            });
        }
        Style::Stroke {
            width,
            join,
            miter_limit,
            start_cap,
            end_cap,
            dashes: dash_bits,
            dash_offset,
        } => {
            dashes = dash_bits
                .iter()
                .map(|value| f32::from_bits(*value))
                .collect();
            mask.style(zeno::Stroke {
                width: f32::from_bits(*width),
                join: zeno_join(*join),
                miter_limit: f32::from_bits(*miter_limit),
                start_cap: zeno_cap(*start_cap),
                end_cap: zeno_cap(*end_cap),
                dashes: &dashes,
                offset: f32::from_bits(*dash_offset),
                scale: true,
            });
        }
    }

    mask.size(size[0], size[1]);
    let (coverage, _) = mask.render();
    coverage
}

/// Appends the commands of `path`, each point mapped by `map`, to `out`.
pub(crate) fn commands_of(
    path: &BezPath,
    map: impl Fn(Point) -> [f32; 2],
    out: &mut Vec<zeno::Command>,
) {
    let point = |at: Point| {
        let [x, y] = map(at);
        zeno::Point::new(x, y)
    };
    for element in path.elements() {
        out.push(match *element {
            PathEl::MoveTo(p) => zeno::Command::MoveTo(point(p)),
            PathEl::LineTo(p) => zeno::Command::LineTo(point(p)),
            PathEl::QuadTo(a, b) => zeno::Command::QuadTo(point(a), point(b)),
            PathEl::CurveTo(a, b, c) => zeno::Command::CurveTo(point(a), point(b), point(c)),
            PathEl::ClosePath => zeno::Command::Close,
        });
    }
}

fn join_code(join: Join) -> u8 {
    match join {
        Join::Bevel => 0,
        Join::Miter => 1,
        Join::Round => 2,
    }
}

fn zeno_join(code: u8) -> zeno::Join {
    match code {
        0 => zeno::Join::Bevel,
        2 => zeno::Join::Round,
        _ => zeno::Join::Miter,
    }
}

fn cap_code(cap: Cap) -> u8 {
    match cap {
        Cap::Butt => 0,
        Cap::Square => 1,
        Cap::Round => 2,
    }
}

fn zeno_cap(code: u8) -> zeno::Cap {
    match code {
        1 => zeno::Cap::Square,
        2 => zeno::Cap::Round,
        _ => zeno::Cap::Butt,
    }
}
