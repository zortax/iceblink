//! The names an assembled application shader shares with its snippet.

use std::collections::BTreeSet;

use iceblink::ShaderMode;
use iceblink::shader::{RESERVED_PREFIX, entry, validation_module};
use iceblink_wgsl::effect;

const MODES: [ShaderMode; 3] = [ShaderMode::PAINT, ShaderMode::COVERAGE, ShaderMode::FILTER];

/// Returns a snippet of `mode` that uses nothing but the prelude.
fn snippet(mode: ShaderMode) -> &'static str {
    match mode {
        ShaderMode::PAINT => {
            "fn shade(input: ShaderInput, params: Params) -> vec4<f32> { return vec4<f32>(1.0); }"
        }
        ShaderMode::COVERAGE => {
            "fn coverage(input: ShaderInput, params: Params) -> f32 { return 1.0; }"
        }
        _ => {
            "fn apply(input: ShaderInput, params: Params, source: texture_2d<f32>, \
             source_sampler: sampler, region: FilterSource) -> vec4<f32> { \
             return source_at(source, source_sampler, region, input.local); }"
        }
    }
}

fn parsed(source: &str) -> naga::Module {
    naga::front::wgsl::parse_str(source).unwrap_or_else(|error| {
        panic!("{}", error.emit_to_string(source));
    })
}

fn validated(source: &str) -> Result<(), String> {
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.message().to_owned())?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// Returns every module-scope name of `module`, leaving out the ones naga declares itself.
fn names(module: &naga::Module) -> BTreeSet<String> {
    let types = module.types.iter().filter_map(|(_, ty)| ty.name.clone());
    let constants = module.constants.iter().filter_map(|(_, c)| c.name.clone());
    let overrides = module.overrides.iter().filter_map(|(_, o)| o.name.clone());
    let globals = module
        .global_variables
        .iter()
        .filter_map(|(_, g)| g.name.clone());
    let functions = module.functions.iter().filter_map(|(_, f)| f.name.clone());
    let entries = module.entry_points.iter().map(|entry| entry.name.clone());
    types
        .chain(constants)
        .chain(overrides)
        .chain(globals)
        .chain(functions)
        .chain(entries)
        .filter(|name| !name.starts_with("__"))
        .collect()
}

#[test]
fn every_name_the_renderer_adds_to_a_shader_is_reserved() {
    for mode in MODES {
        let contract = names(&parsed(&validation_module(mode, snippet(mode)).unwrap()));
        let assembled = names(&parsed(&effect(mode, snippet(mode)).unwrap()));
        let shared: Vec<&String> = assembled
            .iter()
            .filter(|name| !name.starts_with(RESERVED_PREFIX) && !contract.contains(*name))
            .collect();
        assert!(shared.is_empty(), "{mode:?} adds {shared:?}");
        assert!(contract.contains(entry(mode).unwrap()), "{mode:?}");
    }
}

#[test]
fn a_snippet_that_reads_a_renderer_internal_fails_client_validation() {
    let reads_internal = "fn shade(input: ShaderInput, params: Params) -> vec4<f32> {\n    \
                          return vec4<f32>(cr_srgb_to_linear(vec3<f32>(0.5)), 1.0);\n}\n";
    assert_eq!(
        validated(&effect(ShaderMode::PAINT, reads_internal).unwrap()),
        Ok(()),
        "the reference renderer compiles it"
    );
    let refused = validated(&validation_module(ShaderMode::PAINT, reads_internal).unwrap());
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("cr_srgb_to_linear")),
        "{refused:?}"
    );
}
