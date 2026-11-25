//! The quads, shader quads, shadow and line of the scene.

use iceblink::{
    BorderStyle, ChunkRecorder, ClipRef, Color, Line, LineStyle, PaintRef, ROUND, ShaderQuad,
    Shadow, SpaceRef,
};

use super::Refs;
use crate::common::naive::quad;
use crate::common::naive::scene::resources::Resources;

/// Records a dashed, bordered quad, a shader quad of each drawing mode, a shadow and a wavy line.
pub(super) fn record(recorder: &mut ChunkRecorder<'_>, refs: &Refs, resources: &Resources) {
    let mut bordered = quad([2.0, 40.0, 30.0, 20.0], [0, 0, 0, 0]);
    bordered.fill = refs.gradient;
    bordered.stroke = refs.sides;
    bordered.border = [2.0, 3.0, 4.0, 1.0];
    bordered.radii = [6.0; 8];
    bordered.style = BorderStyle::DASHED;
    bordered.clip = refs.local;
    recorder.quad(bordered);

    let paint_slot = recorder.shader_slot(resources.paint_shader);
    let coverage_slot = recorder.shader_slot(resources.coverage_shader);
    let params_slot = recorder.params_slot(resources.params);
    recorder.shader_quad(shader_quad(paint_slot, params_slot));
    recorder.shader_quad(shader_quad(coverage_slot, params_slot));

    recorder.shadow(Shadow {
        shape_bounds: [10.0, 10.0, 30.0, 20.0],
        radii: [4.0; 8],
        caster_bounds: [10.0, 10.0, 30.0, 20.0],
        caster_radii: [4.0; 8],
        shapes: ROUND,
        color: Color([0.0, 0.0, 0.0, 0.6]),
        blur: 3.0,
        clip: refs.clip,
        space: refs.turned,
        inset: 0,
    });
    recorder.line(Line {
        bounds: [0.0, 20.0, 90.0, 4.0],
        color: Color([0.0, 0.0, 1.0, 1.0]),
        thickness: 1.0,
        clip: ClipRef::NONE,
        space: refs.sticky,
        style: LineStyle::WAVY,
        _pad: [0; 3],
    });
}

/// A bordered shader quad drawn by `shader` with `params`.
fn shader_quad(shader: u32, params: u32) -> ShaderQuad {
    ShaderQuad {
        bounds: [4.0, 40.0, 20.0, 20.0],
        radii: [3.0; 8],
        shapes: ROUND,
        border: [1.0; 4],
        fill: PaintRef::color([200, 0, 0, 255]),
        stroke: PaintRef::color([0, 0, 0, 255]),
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        shader,
        params,
        opacity: 0.9,
    }
}
