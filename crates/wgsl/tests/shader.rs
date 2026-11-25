//! The prelude and the module a client validates an application shader with.

use iceblink::ShaderMode;
use iceblink::shader::{validation_module, with_prelude};

fn validated(mode: ShaderMode, snippet: &str) -> Result<(), String> {
    let source = validation_module(mode, snippet).ok_or("the mode is not defined")?;
    let module =
        naga::front::wgsl::parse_str(&source).map_err(|error| error.emit_to_string(&source))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn a_snippet_of_every_mode_that_follows_the_contract_validates() {
    let snippets = [
        (
            ShaderMode::PAINT,
            "struct Params {\n    tint: vec4<f32>,\n}\n\
             fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    \
             return premultiplied(hsl(input.time, 1.0, 0.5), params.tint.a);\n}\n",
        ),
        (
            ShaderMode::COVERAGE,
            "fn coverage(input: ShaderInput, params: Params) -> f32 {\n    \
             return coverage_of(superellipse_sdf(input.local - input.size * 0.5, input.size * 0.5, 4.0));\n}\n",
        ),
        (
            ShaderMode::FILTER,
            "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, \
             source_sampler: sampler, region: FilterSource) -> vec4<f32> {\n    \
             return source_at(source, source_sampler, region, input.local);\n}\n",
        ),
    ];
    for (mode, snippet) in snippets {
        assert_eq!(validated(mode, snippet), Ok(()), "{mode:?}");
    }
}

#[test]
fn a_function_with_the_wrong_signature_fails() {
    let refused = validated(
        ShaderMode::PAINT,
        "fn shade(input: ShaderInput) -> vec4<f32> {\n    return vec4<f32>(1.0);\n}\n",
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("ArgumentCount")),
        "{refused:?}"
    );
}

#[test]
fn parameters_larger_than_the_block_fail() {
    let refused = validated(
        ShaderMode::PAINT,
        "struct Params {\n    values: array<vec4<f32>, 5>,\n}\n\
         fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    \
         return params.values[0];\n}\n",
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("must be at least 80")),
        "{refused:?}"
    );
}

#[test]
fn only_a_snippet_without_parameters_is_given_the_type() {
    let declared = "struct Params {\n    amount: f32,\n}\n";
    assert_eq!(with_prelude(declared).matches("struct Params").count(), 1);
    assert!(with_prelude("fn shade() {}").contains("unused: vec4<f32>"));
    assert!(
        with_prelude("struct ParamsExtra {\n    amount: f32,\n}\n").contains("unused: vec4<f32>")
    );
}

#[test]
fn parameters_outside_the_uniform_layout_fail() {
    let refused = validated(
        ShaderMode::PAINT,
        "struct Params {\n    values: array<f32, 4>,\n}\n\
         fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    \
         return vec4<f32>(params.values[0]);\n}\n",
    );
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("ArrayStride")),
        "{refused:?}"
    );
}
