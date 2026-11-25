//! A drop shadow: the content's alpha, blurred, moved and tinted, drawn beneath it.

use iceblink::geom::{Device, Rect};

use crate::filter::blur;
use crate::filter::{EdgeMode, ShadowLayer};
use crate::frame::build::PlanBuilder;
use crate::frame::target::TargetRef;
use crate::pipeline::composite::CompositeParams;

/// The look of a drop shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shadow {
    /// The displacement, in device pixels.
    pub(crate) offset: (f32, f32),
    /// The standard deviation of the blur.
    pub(crate) deviation: f32,
    /// The premultiplied colour.
    pub(crate) color: [f32; 4],
}

/// Plans the drop shadow of `source` over `output`, and returns the layer to draw behind the
/// content.
///
/// Reads outside `valid`, the device pixels `source` holds, are treated as `edge` says.
///
/// Returns `None` when the blur cannot acquire its targets.
pub(crate) fn plan(
    builder: &mut PlanBuilder<'_>,
    source: TargetRef,
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
    shadow: Shadow,
    edge: EdgeMode,
) -> Option<ShadowLayer> {
    let blurred = blur::plan(builder, source, output, valid, shadow.deviation, edge)?;
    let params = CompositeParams::new(
        output.to_unit(),
        builder.extent_of(blurred.target),
        blurred.target.scale(),
        blurred.target.origin(),
        output,
        iceblink::FRAME_NO_CLIP,
    )
    .as_shadow(shadow.color, shadow.offset);
    // A read past the written region lies beyond the blur's reach of the group's content.
    let params = match edge {
        EdgeMode::Transparent => params.transparent_outside(),
        EdgeMode::Duplicate => params,
    };
    Some(ShadowLayer {
        source: blurred.target,
        params,
    })
}
