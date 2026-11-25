//! Shader quads in paint and coverage mode, and a filter shader over a group.

use std::sync::Arc;

use iceblink::{
    ClipRef, FRAME_NO_CLIP, FrameFilter, FrameGroup, PaintRef, ParamsBlock, ROUND, ShaderDesc,
    ShaderMode, ShaderOp, ShaderQuad, ShaderReads, ShaderSource, SpaceRef,
};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, group, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden, skip};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Shaded colours and edges: a pixel may differ by a few steps where coverage is partial.
const EDGES: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 32,
};

const PAINT: &str = "fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    return vec4<f32>(input.uv.x, input.uv.y, 0.5, 1.0);\n}\n";

const COVERAGE: &str = "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    return coverage_of(length(input.local - input.size * 0.5) - min(input.size.x, input.size.y) * 0.4);\n}\n";

const FILTER: &str = "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    let texel = source_at(source, source_sampler, region, input.local);\n    return vec4<f32>(texel.b, texel.g, texel.r, texel.a);\n}\n";

fn desc(mode: ShaderMode, wgsl: &str) -> Arc<ShaderDesc> {
    Arc::new(ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from(wgsl),
            prebuilt: None,
        },
        params_size: 0,
        reads: ShaderReads::default(),
    })
}

fn shader_quad(bounds: [f32; 4], shader: u32, params: u32, fill: PaintRef) -> ShaderQuad {
    ShaderQuad {
        bounds,
        radii: [8.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        shader,
        params,
        opacity: 1.0,
    }
}

/// A paint shader cut by rounded corners, a coverage shader filled with a paint, a faded paint
/// shader, and a filter shader that swaps a group's red and blue.
pub fn shader_quads(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    if !canvas.caps().custom_shaders {
        skip("shader_quads", "the renderer runs no application shaders");
        return;
    }
    let mut builder = FrameBuilder::new(SIZE);
    builder.shader(ShaderOp::Create {
        shader: 0,
        desc: desc(ShaderMode::PAINT, PAINT),
    });
    builder.shader(ShaderOp::Create {
        shader: 1,
        desc: desc(ShaderMode::COVERAGE, COVERAGE),
    });
    builder.shader(ShaderOp::Create {
        shader: 2,
        desc: desc(ShaderMode::FILTER, FILTER),
    });
    let params = builder.params(ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let green = builder.solid(opaque(20, 160, 60));

    builder.shader_quad(shader_quad(cell(0), 0, params, PaintRef::NONE));
    builder.shader_quad(shader_quad(cell(1), 1, params, green));
    builder.shader_quad(ShaderQuad {
        opacity: 0.5,
        ..shader_quad(cell(2), 0, params, PaintRef::NONE)
    });

    let red = builder.solid(opaque(220, 30, 30));
    builder.begin_group(FrameGroup {
        filters: Arc::from(vec![FrameFilter::Shader {
            shader: 2,
            params,
            reach: 0.0,
        }]),
        ..group(cell(3), 1.0, &[])
    });
    builder.quad(filled(cell(3), red));
    builder.end_group();

    golden("shader_quads", &canvas.draw(builder.finish()), EDGES);
}
