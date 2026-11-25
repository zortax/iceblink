//! Mask coverage rasterised with zeno.

use crate::naive::content::masks::{Mask, MaskCommand, MaskStyle, StrokeCap, StrokeJoin};

/// Returns the coverage of `mask`, one byte per texel with rows tightly packed.
pub(super) fn coverage(mask: &Mask) -> Vec<u8> {
    let point = |bits: [u32; 2]| zeno::Point::new(f32::from_bits(bits[0]), f32::from_bits(bits[1]));
    let commands: Vec<zeno::Command> = mask
        .commands
        .iter()
        .map(|command| match *command {
            MaskCommand::Move(p) => zeno::Command::MoveTo(point(p)),
            MaskCommand::Line(p) => zeno::Command::LineTo(point(p)),
            MaskCommand::Quad(p) => zeno::Command::QuadTo(point([p[0], p[1]]), point([p[2], p[3]])),
            MaskCommand::Curve(p) => zeno::Command::CurveTo(
                point([p[0], p[1]]),
                point([p[2], p[3]]),
                point([p[4], p[5]]),
            ),
            MaskCommand::Close => zeno::Command::Close,
        })
        .collect();

    let mut raster = zeno::Mask::new(&commands);
    let dashes: Vec<f32>;
    match &mask.style {
        MaskStyle::Fill { even_odd } => {
            raster.style(if *even_odd {
                zeno::Fill::EvenOdd
            } else {
                zeno::Fill::NonZero
            });
        }
        MaskStyle::Stroke {
            width,
            join,
            miter_limit,
            caps,
            dashes: dash_bits,
            dash_offset,
        } => {
            dashes = dash_bits.iter().map(|bits| f32::from_bits(*bits)).collect();
            raster.style(zeno::Stroke {
                width: f32::from_bits(*width),
                join: match join {
                    StrokeJoin::Bevel => zeno::Join::Bevel,
                    StrokeJoin::Miter => zeno::Join::Miter,
                    StrokeJoin::Round => zeno::Join::Round,
                },
                miter_limit: f32::from_bits(*miter_limit),
                start_cap: cap(caps[0]),
                end_cap: cap(caps[1]),
                dashes: &dashes,
                offset: f32::from_bits(*dash_offset),
                scale: true,
            });
        }
    }

    raster.size(mask.size[0], mask.size[1]);
    raster.render().0
}

fn cap(cap: StrokeCap) -> zeno::Cap {
    match cap {
        StrokeCap::Butt => zeno::Cap::Butt,
        StrokeCap::Square => zeno::Cap::Square,
        StrokeCap::Round => zeno::Cap::Round,
    }
}
