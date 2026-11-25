//! The block of per-target values every pipeline binds.

use bytemuck::{Pod, Zeroable};
use iceblink::geom::{Device, Point, Size};
use iceblink::{ShaderClock, SubpixelOrder};

use crate::target::scale::TargetScale;

/// The display gamma text coverage is corrected for.
pub(crate) const DISPLAY_GAMMA: f32 = 2.2;

/// The contrast enhancement of single-channel text coverage.
pub(crate) const GRAYSCALE_CONTRAST: f32 = 0.5;

/// The contrast enhancement of per-channel text coverage.
pub(crate) const SUBPIXEL_CONTRAST: f32 = 1.0;

/// The globals block, laid out as the shaders' `cr_Globals`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct Globals {
    /// The target's extent in texels, then its texels per device pixel on each axis.
    pub(crate) viewport: [f32; 4],
    /// The gamma-correction coefficients text coverage is corrected with.
    pub(crate) gamma_ratios: [f32; 4],
    /// The two contrast enhancements, then 1 when the display's stripes run blue to red.
    pub(crate) text: [f32; 4],
    /// The shader clock's time and delta, then the output's scale factor.
    pub(crate) frame: [f32; 4],
    /// The device pixel the target's first texel holds.
    pub(crate) origin: [f32; 4],
}

impl Globals {
    /// Returns the globals of a target of `size` device pixels held at `scale`, on a display whose
    /// stripes run in `order`.
    ///
    /// The target starts at the device origin, and the clock and the output's scale are zero.
    pub(crate) fn for_target(
        size: Size<i32, Device>,
        scale: TargetScale,
        order: SubpixelOrder,
    ) -> Self {
        let extent = scale.extent(size);
        Self {
            viewport: [
                extent.width.max(1) as f32,
                extent.height.max(1) as f32,
                scale.factor(),
                scale.factor(),
            ],
            gamma_ratios: iceblink::color::gamma_correction_ratios(DISPLAY_GAMMA),
            text: [
                GRAYSCALE_CONTRAST,
                SUBPIXEL_CONTRAST,
                f32::from(order == SubpixelOrder::BlueToRed),
                0.0,
            ],
            frame: [0.0; 4],
            origin: [0.0; 4],
        }
    }

    /// Returns the globals with the target's first texel at device pixel `origin`.
    pub(crate) fn with_origin(mut self, origin: Point<i32, Device>) -> Self {
        self.origin = [origin.x as f32, origin.y as f32, 0.0, 0.0];
        self
    }

    /// Returns the globals with the shader clock and the output's scale factor.
    pub(crate) fn with_frame(mut self, clock: ShaderClock, scale: f32) -> Self {
        self.frame = [clock.time, clock.delta, scale, 0.0];
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Globals, SubpixelOrder};
    use crate::target::scale::TargetScale;
    use iceblink::geom::Size;

    #[test]
    fn the_target_extent_is_never_zero() {
        let globals = Globals::for_target(
            Size::new(0, -4),
            TargetScale::Full,
            SubpixelOrder::RedToBlue,
        );
        assert_eq!(globals.viewport[0], 1.0);
        assert_eq!(globals.viewport[1], 1.0);
    }

    #[test]
    fn the_subpixel_order_reaches_the_shader_as_a_flag() {
        assert_eq!(
            Globals::for_target(Size::new(8, 8), TargetScale::Full, SubpixelOrder::RedToBlue).text
                [2],
            0.0
        );
        assert_eq!(
            Globals::for_target(Size::new(8, 8), TargetScale::Full, SubpixelOrder::BlueToRed).text
                [2],
            1.0
        );
    }

    #[test]
    fn a_half_resolution_target_halves_its_extent_and_says_so() {
        let full = Globals::for_target(
            Size::new(64, 32),
            TargetScale::Full,
            SubpixelOrder::default(),
        );
        let half = Globals::for_target(
            Size::new(64, 32),
            TargetScale::Half,
            SubpixelOrder::default(),
        );
        assert_eq!(full.viewport, [64.0, 32.0, 1.0, 1.0]);
        assert_eq!(half.viewport, [32.0, 16.0, 0.5, 0.5]);
        // The two lanes are what the shader divides by to recover a device pixel, so a target
        // whose extent halved and whose scale did not would place every clip twice as far out.
        assert_eq!(
            half.viewport[0] * (1.0 / half.viewport[2]),
            full.viewport[0]
        );
    }

    #[test]
    fn a_target_origin_reaches_the_shader_and_defaults_to_the_surface_origin() {
        use iceblink::geom::Point;

        let globals = Globals::for_target(
            Size::new(64, 32),
            TargetScale::Full,
            SubpixelOrder::default(),
        );
        assert_eq!(globals.origin, [0.0; 4]);
        let moved = globals.with_origin(Point::new(-6, 40));
        assert_eq!(moved.origin, [-6.0, 40.0, 0.0, 0.0]);
        assert_eq!(
            moved.viewport, globals.viewport,
            "the origin moves no extent"
        );
    }

    #[test]
    fn the_correction_is_the_one_computed_for_the_display_gamma() {
        let globals =
            Globals::for_target(Size::new(8, 8), TargetScale::Full, SubpixelOrder::default());
        assert_eq!(
            globals.gamma_ratios,
            iceblink::color::gamma_correction_ratios(super::DISPLAY_GAMMA)
        );
    }
}
