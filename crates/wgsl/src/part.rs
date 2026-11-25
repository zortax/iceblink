//! The named parts modules are concatenated from.

/// The names of all parts, in alphabetical order.
pub const PART_NAMES: [&str; 27] = [
    "blit",
    "blur",
    "clear",
    "common",
    "composite",
    "external",
    "line",
    "marks",
    "marks_box",
    "marks_composite",
    "marks_disc",
    "marks_glyph",
    "marks_polyline",
    "paint",
    "quad",
    "sdf",
    "shader_coverage",
    "shader_filter",
    "shader_paint",
    "shader_quad",
    "shadow",
    "sprite",
    "sprite_color",
    "sprite_mono",
    "subpixel",
    "text",
    "vector",
];

/// Returns the text of the part called `name`.
///
/// # Panics
///
/// Panics when no part has that name.
pub fn part(name: &str) -> &'static str {
    match name {
        "blit" => include_str!("wgsl/blit.wgsl"),
        "blur" => include_str!("wgsl/blur.wgsl"),
        "clear" => include_str!("wgsl/clear.wgsl"),
        "common" => include_str!("wgsl/common.wgsl"),
        "composite" => include_str!("wgsl/composite.wgsl"),
        "external" => include_str!("wgsl/external.wgsl"),
        "line" => include_str!("wgsl/line.wgsl"),
        "marks" => include_str!("wgsl/marks.wgsl"),
        "marks_box" => include_str!("wgsl/marks_box.wgsl"),
        "marks_composite" => include_str!("wgsl/marks_composite.wgsl"),
        "marks_disc" => include_str!("wgsl/marks_disc.wgsl"),
        "marks_glyph" => include_str!("wgsl/marks_glyph.wgsl"),
        "marks_polyline" => include_str!("wgsl/marks_polyline.wgsl"),
        "paint" => include_str!("wgsl/paint.wgsl"),
        "quad" => include_str!("wgsl/quad.wgsl"),
        "sdf" => include_str!("wgsl/sdf.wgsl"),
        "shader_coverage" => include_str!("wgsl/shader_coverage.wgsl"),
        "shader_filter" => include_str!("wgsl/shader_filter.wgsl"),
        "shader_paint" => include_str!("wgsl/shader_paint.wgsl"),
        "shader_quad" => include_str!("wgsl/shader_quad.wgsl"),
        "shadow" => include_str!("wgsl/shadow.wgsl"),
        "sprite" => include_str!("wgsl/sprite.wgsl"),
        "sprite_color" => include_str!("wgsl/sprite_color.wgsl"),
        "sprite_mono" => include_str!("wgsl/sprite_mono.wgsl"),
        "subpixel" => include_str!("wgsl/subpixel.wgsl"),
        "text" => include_str!("wgsl/text.wgsl"),
        "vector" => include_str!("wgsl/vector.wgsl"),
        other => panic!("no part is called `{other}`"),
    }
}
