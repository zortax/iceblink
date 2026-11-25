//! The block a filter shader runs with.

use bytemuck::{Pod, Zeroable};
use iceblink::ShaderClock;
use iceblink::geom::{Device, Point, Rect, Size};

use crate::target::scale::TargetScale;

/// The uniform block of a filter shader pass.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct EffectFilterParams {
    pub(crate) extents: [f32; 4],
    pub(crate) placement: [f32; 4],
    pub(crate) valid: [f32; 4],
    pub(crate) group: [f32; 4],
    pub(crate) frame: [f32; 4],
    pub(crate) origins: [f32; 4],
}

impl EffectFilterParams {
    /// Returns the block of a pass over `region` from `source` into `destination`, each given as
    /// extent, scale and origin, at the shader time `clock` and the display scale `scale`.
    pub(crate) fn new(
        region: Rect<i32, Device>,
        source: (Size<i32, Device>, TargetScale, Point<i32, Device>),
        destination: (Size<i32, Device>, TargetScale, Point<i32, Device>),
        clock: ShaderClock,
        scale: f32,
    ) -> Self {
        let (source_extent, source_scale, source_origin) = source;
        let (destination_extent, destination_scale, destination_origin) = destination;
        let read = region.translate(Point::ORIGIN.offset_from(source_origin));
        let valid = [
            read.origin.x as f32 * source_scale.factor(),
            read.origin.y as f32 * source_scale.factor(),
            (read.origin.x + read.size.width) as f32 * source_scale.factor(),
            (read.origin.y + read.size.height) as f32 * source_scale.factor(),
        ];
        Self {
            extents: [
                source_extent.width.max(1) as f32,
                source_extent.height.max(1) as f32,
                destination_extent.width.max(1) as f32,
                destination_extent.height.max(1) as f32,
            ],
            placement: [
                region.origin.x as f32,
                region.origin.y as f32,
                source_scale.factor(),
                destination_scale.factor(),
            ],
            valid,
            group: [
                region.size.width as f32,
                region.size.height as f32,
                clock.time,
                clock.delta,
            ],
            frame: [scale, 0.0, 0.0, 0.0],
            origins: [
                source_origin.x as f32,
                source_origin.y as f32,
                destination_origin.x as f32,
                destination_origin.y as f32,
            ],
        }
    }
}
