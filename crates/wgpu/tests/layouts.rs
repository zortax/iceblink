//! The renderer's shader structures and loaders agree with the Rust layouts it writes.

use std::sync::Arc;

use iceblink::{ShaderDesc, ShaderMode, ShaderReads, ShaderSource};

#[test]
fn every_shader_structure_and_loader_agrees_with_the_rust_layout() {
    iceblink_wgpu::check_shader_layouts().expect("the shader and Rust layouts agree");
}

#[test]
fn an_application_shader_that_declares_too_many_parameters_is_refused() {
    let desc = |params_size| ShaderDesc {
        mode: ShaderMode::PAINT,
        source: ShaderSource {
            wgsl: Arc::from(
                "struct Params {\n    a: vec4<f32>,\n    b: vec4<f32>,\n}\nfn shade(input: ShaderInput, params: Params) -> vec4<f32> { return params.a; }\n",
            ),
            prebuilt: None,
        },
        params_size,
        reads: ShaderReads::default(),
    };
    assert!(iceblink_wgpu::check_shader(&desc(32)).is_ok());
    assert!(
        iceblink_wgpu::check_shader(&desc(16)).is_err(),
        "Params is wider than declared"
    );
    assert!(
        iceblink_wgpu::check_shader(&desc(80)).is_err(),
        "wider than the block"
    );
}
