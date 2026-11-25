//! The declarations a validation module checks a snippet with.

use crate::ShaderMode;

use super::MAX_PARAMS_BYTES;

/// Returns the parameter block, which holds `Params` the way a renderer binds it.
pub(super) fn params_block() -> String {
    format!(
        "struct cr_ParamsBlock {{\n    pointer: vec2<f32>,\n    hovered: f32,\n    reserved: f32,\n    \
         @size({MAX_PARAMS_BYTES}) user: Params,\n}}\n\n\
         @group(0) @binding(0) var<uniform> cr_params: cr_ParamsBlock;\n"
    )
}

/// Returns a function that calls the snippet's function of `mode` with the arguments the mode
/// passes and returns its result as the type the mode expects.
///
/// Returns `None` for a mode the contract does not define.
pub(super) fn signature(mode: ShaderMode) -> Option<&'static str> {
    match mode {
        ShaderMode::PAINT => Some(PAINT),
        ShaderMode::COVERAGE => Some(COVERAGE),
        ShaderMode::FILTER => Some(FILTER),
        _ => None,
    }
}

const PAINT: &str = "\
fn cr_signature(input: ShaderInput, params: Params) -> vec4<f32> {
    return shade(input, params);
}
";

const COVERAGE: &str = "\
fn cr_signature(input: ShaderInput, params: Params) -> f32 {
    return coverage(input, params);
}
";

const FILTER: &str = "\
fn cr_signature(
    input: ShaderInput,
    params: Params,
    source: texture_2d<f32>,
    source_sampler: sampler,
    region: FilterSource,
) -> vec4<f32> {
    return apply(input, params, source, source_sampler, region);
}
";
