//! The shaders of the scene, one of each mode.

use std::sync::Arc;

use iceblink::{ShaderDesc, ShaderMode, ShaderReads, ShaderSource};

/// A shader of `mode` that reads a parameter block of one `f32` and the time.
pub(super) fn shader(mode: ShaderMode) -> ShaderDesc {
    ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from(source(mode)),
            prebuilt: None,
        },
        params_size: 4,
        reads: ShaderReads::TIME,
    }
}

/// Returns WGSL for a shader of `mode`.
fn source(mode: ShaderMode) -> String {
    let function = if mode == ShaderMode::PAINT {
        "fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    \
         return premultiplied(vec3<f32>(input.uv, params.x), 1.0);\n}"
    } else if mode == ShaderMode::COVERAGE {
        "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    \
         return coverage_of(length(input.local - input.size / 2.0) - input.size.x / 3.0);\n}"
    } else {
        "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, \
         source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    \
         return source_at(source, source_sampler, region, region.origin + input.local);\n}"
    };
    format!("struct Params {{ x: f32 }}\n{function}\n")
}
