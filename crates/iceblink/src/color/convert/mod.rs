//! Conversion between the fourteen colour spaces.
//!
//! Every conversion goes through D65-referenced CIE XYZ, except between spaces that are forms of
//! one another: sRGB, HSL and HWB, sRGB and linear sRGB, L\*a\*b\* and LCH, and Oklab and Oklch.

pub(crate) mod adapt;
pub(crate) mod hsl;
pub(crate) mod hwb;
pub(crate) mod lab;
pub(crate) mod matrix;
pub(crate) mod oklab;
pub(crate) mod polar;
pub(crate) mod rgb;

use crate::color::space::{ColorSpace, WhitePoint};

/// Converts three channel values from one space to another.
pub(crate) fn convert(components: [f32; 3], from: ColorSpace, to: ColorSpace) -> [f32; 3] {
    if from == to {
        return components;
    }
    if let Some(direct) = shortcut(components, from, to) {
        return direct;
    }
    from_xyz_d65(to, to_xyz_d65(from, components))
}

/// Converts directly between spaces that are forms of one another, or returns `None` for any other
/// pair.
fn shortcut(components: [f32; 3], from: ColorSpace, to: ColorSpace) -> Option<[f32; 3]> {
    use ColorSpace::{Hsl, Hwb, Lab, Lch, Oklab, Oklch, Srgb, SrgbLinear};
    let converted = match (from, to) {
        (Srgb, Hsl) => hsl::from_srgb(components),
        (Hsl, Srgb) => hsl::to_srgb(components),
        (Srgb, Hwb) => hwb::from_srgb(components),
        (Hwb, Srgb) => hwb::to_srgb(components),
        (Hsl, Hwb) => hwb::from_srgb(hsl::to_srgb(components)),
        (Hwb, Hsl) => hsl::from_srgb(hwb::to_srgb(components)),
        (Srgb, SrgbLinear) => components.map(rgb::transfer::srgb_to_linear),
        (SrgbLinear, Srgb) => components.map(rgb::transfer::linear_to_srgb),
        (Lab, Lch) | (Oklab, Oklch) => polar::to_polar(components),
        (Lch, Lab) | (Oklch, Oklab) => polar::to_rectangular(components),
        _ => return None,
    };
    Some(converted)
}

/// Converts a colour in `space` to D65-referenced CIE XYZ.
fn to_xyz_d65(space: ColorSpace, components: [f32; 3]) -> [f32; 3] {
    let xyz = match space {
        ColorSpace::Hsl => rgb::to_xyz(ColorSpace::Srgb, hsl::to_srgb(components)),
        ColorSpace::Hwb => rgb::to_xyz(ColorSpace::Srgb, hwb::to_srgb(components)),
        ColorSpace::Lab => lab::to_xyz_d50(components),
        ColorSpace::Lch => lab::to_xyz_d50(polar::to_rectangular(components)),
        ColorSpace::Oklab => oklab::to_xyz_d65(components),
        ColorSpace::Oklch => oklab::to_xyz_d65(polar::to_rectangular(components)),
        ColorSpace::XyzD50 | ColorSpace::XyzD65 => components,
        _ => {
            debug_assert!(rgb::is_rgb(space), "{space:?} has no encoder");
            rgb::to_xyz(space, components)
        }
    };
    match space.white_point() {
        WhitePoint::D50 => adapt::d50_to_d65(xyz),
        WhitePoint::D65 => xyz,
    }
}

/// Converts D65-referenced CIE XYZ to a colour in `space`.
fn from_xyz_d65(space: ColorSpace, xyz: [f32; 3]) -> [f32; 3] {
    let xyz = match space.white_point() {
        WhitePoint::D50 => adapt::d65_to_d50(xyz),
        WhitePoint::D65 => xyz,
    };
    match space {
        ColorSpace::Hsl => hsl::from_srgb(rgb::from_xyz(ColorSpace::Srgb, xyz)),
        ColorSpace::Hwb => hwb::from_srgb(rgb::from_xyz(ColorSpace::Srgb, xyz)),
        ColorSpace::Lab => lab::from_xyz_d50(xyz),
        ColorSpace::Lch => polar::to_polar(lab::from_xyz_d50(xyz)),
        ColorSpace::Oklab => oklab::from_xyz_d65(xyz),
        ColorSpace::Oklch => polar::to_polar(oklab::from_xyz_d65(xyz)),
        ColorSpace::XyzD50 | ColorSpace::XyzD65 => xyz,
        _ => {
            debug_assert!(rgb::is_rgb(space), "{space:?} has no decoder");
            rgb::from_xyz(space, xyz)
        }
    }
}
