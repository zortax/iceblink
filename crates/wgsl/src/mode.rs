//! The parts and entry points of each shader mode.

use iceblink::ShaderMode;

/// Reports whether shaders of `mode` draw a rectangle, as paint and coverage shaders do.
pub fn is_rectangle(mode: ShaderMode) -> bool {
    mode == ShaderMode::PAINT || mode == ShaderMode::COVERAGE
}

/// Returns the parts that follow the application's text in a shader of `mode`.
///
/// Returns `None` for a mode the renderer does not know.
pub fn epilogue(mode: ShaderMode) -> Option<&'static [&'static str]> {
    match mode {
        ShaderMode::PAINT => Some(&["shader_quad", "shader_paint"]),
        ShaderMode::COVERAGE => Some(&["shader_quad", "shader_coverage"]),
        ShaderMode::FILTER => Some(&["shader_filter"]),
        _ => None,
    }
}

/// Returns the fragment entry point of a shader of `mode`.
///
/// Returns `None` for a mode the renderer does not know.
pub fn fragment_entry(mode: ShaderMode) -> Option<&'static str> {
    match mode {
        ShaderMode::PAINT => Some("cr_fs_shader_paint"),
        ShaderMode::COVERAGE => Some("cr_fs_shader_coverage"),
        ShaderMode::FILTER => Some("cr_fs_shader_filter"),
        _ => None,
    }
}

/// Returns the vertex entry point of a shader of `mode`.
///
/// Returns `None` for a mode the renderer does not know.
pub fn vertex_entry(mode: ShaderMode) -> Option<&'static str> {
    match mode {
        ShaderMode::PAINT | ShaderMode::COVERAGE => Some("cr_vs_shader_quad"),
        ShaderMode::FILTER => Some("cr_vs_shader_filter"),
        _ => None,
    }
}
