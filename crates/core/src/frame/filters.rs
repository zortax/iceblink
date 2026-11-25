//! Filter chains with frame slots in place of typed ids.

use std::sync::Arc;

use iceblink::{Filter, FrameFilter};

use crate::scene::Scene;
use crate::scene::record::NO_FRAME_SLOT;

/// Converts a filter chain into its frame form.
pub(crate) fn frame_filters(scene: &Scene, filters: &[Filter]) -> Arc<[FrameFilter]> {
    filters
        .iter()
        .map(|filter| match *filter {
            Filter::Blur(deviation) => FrameFilter::Blur(deviation),
            Filter::DropShadow {
                offset,
                std_dev,
                color,
            } => FrameFilter::DropShadow {
                offset,
                std_dev,
                color: color.quantized(),
            },
            Filter::ColorMatrix(matrix) => FrameFilter::ColorMatrix(matrix),
            Filter::Opacity(amount) => FrameFilter::Opacity(amount),
            Filter::Shader {
                shader,
                params,
                reach,
            } => FrameFilter::Shader {
                shader: scene
                    .shaders
                    .at(shader.slot())
                    .map_or(NO_FRAME_SLOT, |record| record.links.frame),
                params: scene
                    .params
                    .at(params.slot())
                    .map_or(NO_FRAME_SLOT, |record| record.links.frame),
                reach,
            },
        })
        .collect()
}
