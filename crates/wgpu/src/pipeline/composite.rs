//! The block a group target is composited with.

use bytemuck::{Pod, Zeroable};
use iceblink::geom::{Device, DevicePx, Point, Rect, Size};

use crate::filter::matrix::ColorMatrix;
use crate::target::scale::TargetScale;

/// The uniform block of a composite of a group target.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct CompositeParams {
    pub(crate) bounds: [f32; 4],
    pub(crate) source: [f32; 4],
    pub(crate) control: [f32; 4],
    pub(crate) tint: [f32; 4],
    pub(crate) matrix0: [f32; 4],
    pub(crate) matrix1: [f32; 4],
    pub(crate) matrix2: [f32; 4],
    pub(crate) matrix3: [f32; 4],
    pub(crate) matrix_offset: [f32; 4],
    pub(crate) origin: [f32; 4],
    pub(crate) written: [f32; 4],
}

impl CompositeParams {
    /// The flag that replaces the sampled colour with `tint` scaled by its alpha.
    pub(crate) const TINT: u32 = 1;
    /// The flag that runs the sampled colour through the matrix.
    pub(crate) const MATRIX: u32 = 2;
    /// The flag that reads transparent outside the written texels.
    pub(crate) const TRANSPARENT: u32 = 4;

    /// Returns the block of a composite over `bounds` from a target of `extent` texels at
    /// `scale` whose first texel holds device pixel `origin`.
    ///
    /// Reads are clamped to the texels that hold the device pixels `written`.
    pub(crate) fn new(
        bounds: Rect<DevicePx, Device>,
        extent: Size<i32, Device>,
        scale: TargetScale,
        origin: Point<i32, Device>,
        written: Rect<i32, Device>,
        clip: u32,
    ) -> Self {
        let identity = ColorMatrix::identity().columns();
        let texels = scale.texels_at(written, origin);
        Self {
            bounds: [
                bounds.origin.x.0,
                bounds.origin.y.0,
                bounds.size.width.0,
                bounds.size.height.0,
            ],
            source: [
                extent.width.max(1) as f32,
                extent.height.max(1) as f32,
                scale.factor(),
                1.0,
            ],
            control: [f32::from_bits(clip), 0.0, 0.0, 0.0],
            tint: [0.0; 4],
            matrix0: identity[0],
            matrix1: identity[1],
            matrix2: identity[2],
            matrix3: identity[3],
            matrix_offset: identity[4],
            origin: [origin.x as f32, origin.y as f32, 0.0, 0.0],
            written: [
                texels.left() as f32,
                texels.top() as f32,
                texels.right() as f32,
                texels.bottom() as f32,
            ],
        }
    }

    /// Returns this block with the composite's alpha multiplied by `opacity`.
    pub(crate) fn with_opacity(mut self, opacity: f32) -> Self {
        self.source[3] = opacity.clamp(0.0, 1.0);
        self
    }

    /// Returns this block applying `matrix`, which an identity leaves out.
    pub(crate) fn with_matrix(mut self, matrix: ColorMatrix) -> Self {
        let columns = matrix.columns();
        self.matrix0 = columns[0];
        self.matrix1 = columns[1];
        self.matrix2 = columns[2];
        self.matrix3 = columns[3];
        self.matrix_offset = columns[4];
        if !matrix.is_identity() {
            self.control[1] = (self.flags() | Self::MATRIX) as f32;
        }
        self
    }

    /// Returns this block drawing a shadow of `color`, displaced by `offset` device pixels.
    pub(crate) fn as_shadow(mut self, color: [f32; 4], offset: (f32, f32)) -> Self {
        self.tint = color;
        self.control[1] = (self.flags() | Self::TINT) as f32;
        self.control[2] = offset.0;
        self.control[3] = offset.1;
        self
    }

    /// Returns this block reading transparent outside the written texels.
    pub(crate) fn transparent_outside(mut self) -> Self {
        self.control[1] = (self.flags() | Self::TRANSPARENT) as f32;
        self
    }

    pub(crate) fn flags(&self) -> u32 {
        self.control[1] as u32
    }
}

#[cfg(test)]
mod tests {
    use super::CompositeParams;
    use crate::filter::matrix::ColorMatrix;
    use crate::target::scale::TargetScale;
    use iceblink::geom::{Device, DevicePx, Point, Rect, Size};

    fn bounds() -> Rect<DevicePx, Device> {
        Rect::new(
            Point::new(DevicePx(4.0), DevicePx(8.0)),
            Size::new(DevicePx(16.0), DevicePx(32.0)),
        )
    }

    #[test]
    fn a_group_with_no_per_pixel_filter_does_not_ask_for_the_matrix() {
        let params = CompositeParams::new(
            bounds(),
            Size::new(64, 64),
            TargetScale::Full,
            Point::new(0, 0),
            Rect::new(Point::new(0, 0), Size::new(64, 64)),
            0,
        )
        .with_matrix(ColorMatrix::identity());
        assert_eq!(params.flags() & CompositeParams::MATRIX, 0);
        assert_eq!(params.matrix0, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_real_filter_asks_for_the_matrix_and_carries_its_columns() {
        let params = CompositeParams::new(
            bounds(),
            Size::new(64, 64),
            TargetScale::Full,
            Point::new(0, 0),
            Rect::new(Point::new(0, 0), Size::new(64, 64)),
            0,
        )
        .with_matrix(
            ColorMatrix::of(iceblink::FrameFilter::ColorMatrix([
                -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ]))
            .unwrap(),
        );
        assert_ne!(params.flags() & CompositeParams::MATRIX, 0);
        assert_eq!(params.matrix_offset, [1.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn a_shadow_carries_its_colour_and_its_displacement() {
        let params = CompositeParams::new(
            bounds(),
            Size::new(64, 64),
            TargetScale::Half,
            Point::new(0, 0),
            Rect::new(Point::new(0, 0), Size::new(64, 64)),
            3,
        )
        .as_shadow([0.0, 0.0, 0.0, 0.5], (4.0, -2.0));
        assert_ne!(params.flags() & CompositeParams::TINT, 0);
        assert_eq!(params.control[2], 4.0);
        assert_eq!(params.control[3], -2.0);
        assert_eq!(
            params.source[2], 0.5,
            "a half-resolution target is magnified"
        );
        assert_eq!(
            params.control[0].to_bits(),
            3,
            "the clip slot reaches the shader"
        );
    }

    #[test]
    fn the_sampled_target_origin_reaches_the_shader() {
        let params = CompositeParams::new(
            bounds(),
            Size::new(64, 64),
            TargetScale::Full,
            Point::new(32, -8),
            Rect::new(Point::new(32, -8), Size::new(64, 64)),
            0,
        );
        assert_eq!(params.origin, [32.0, -8.0, 0.0, 0.0]);
        assert_eq!(
            params.source[..2],
            [64.0, 64.0],
            "the origin moves no extent"
        );
    }
}
