//! Compositing a filtered target into the target beneath.

use iceblink::geom::{Device, Rect};

use crate::filter::Filtered;
use crate::frame::build::PlanBuilder;
use crate::frame::segment::PlannedDraw;
use crate::frame::target::TargetRef;
use crate::pipeline::composite::CompositeParams;

/// Plans the composite of `filtered` into `destination` over `bounds`, its shadows first, with
/// the frame clip slot `clip` and `opacity`.
pub(super) fn composite(
    builder: &mut PlanBuilder<'_>,
    filtered: &Filtered,
    destination: TargetRef,
    scissor: Rect<i32, Device>,
    bounds: Rect<i32, Device>,
    clip: u32,
    opacity: f32,
) {
    builder.begin_pass(destination, scissor);
    for shadow in &filtered.shadows {
        let mut params = shadow.params;
        params.bounds = rect_bounds(bounds);
        params.control[0] = f32::from_bits(clip);
        params = params.with_opacity(opacity);
        let staged = builder.stage_composite(&params);
        builder.draw(PlannedDraw::Composite {
            source: shadow.source,
            params: staged,
        });
    }

    let params = CompositeParams::new(
        bounds.to_unit(),
        builder.extent_of(filtered.target),
        filtered.target.scale(),
        filtered.target.origin(),
        filtered.holds,
        clip,
    )
    .with_opacity(opacity)
    .with_matrix(filtered.matrix);
    let staged = builder.stage_composite(&params);
    builder.draw(PlannedDraw::Composite {
        source: filtered.target,
        params: staged,
    });
}

/// Releases the lent targets of `filtered`: its shadows' sources, and then its own target.
pub(super) fn release_filtered(builder: &mut PlanBuilder<'_>, filtered: &Filtered) {
    for slot in filtered
        .shadows
        .iter()
        .filter_map(|shadow| shadow.source.slot())
        .chain(filtered.target.slot())
    {
        builder.release(slot);
    }
}

/// Returns `rect` as `[x, y, width, height]`.
fn rect_bounds(rect: Rect<i32, Device>) -> [f32; 4] {
    [
        rect.origin.x as f32,
        rect.origin.y as f32,
        rect.size.width as f32,
        rect.size.height as f32,
    ]
}
