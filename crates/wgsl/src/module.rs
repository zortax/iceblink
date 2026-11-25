//! The renderer's own modules.

use crate::part::part;

/// Every module of the renderer, as its name and the parts it is concatenated from.
pub const MODULES: &[(&str, &[&str])] = &[
    ("quad", &["common", "sdf", "paint", "quad"]),
    ("shadow", &["common", "sdf", "paint", "shadow"]),
    ("line", &["common", "sdf", "paint", "line"]),
    (
        "mono_sprite",
        &["common", "sdf", "paint", "text", "sprite", "sprite_mono"],
    ),
    (
        "color_sprite",
        &["common", "sdf", "paint", "text", "sprite", "sprite_color"],
    ),
    (
        "subpixel_sprite",
        &["common", "sdf", "paint", "text", "sprite", "subpixel"],
    ),
    ("blit", &["blit"]),
    ("clear", &["clear"]),
    ("blur", &["blur"]),
    ("composite", &["common", "sdf", "composite"]),
    ("external", &["common", "sdf", "external"]),
    ("vector", &["common", "sdf", "vector"]),
    (
        "marks_disc",
        &["common", "sdf", "paint", "marks", "marks_disc"],
    ),
    (
        "marks_box",
        &["common", "sdf", "paint", "marks", "marks_box"],
    ),
    (
        "marks_polyline",
        &["common", "sdf", "paint", "marks", "marks_polyline"],
    ),
    (
        "marks_glyph",
        &["common", "sdf", "paint", "marks", "marks_glyph"],
    ),
    (
        "marks_composite",
        &["common", "sdf", "paint", "marks", "marks_composite"],
    ),
];

/// Directives that precede a module's parts, as the module's name and the text.
pub const PRELUDES: &[(&str, &str)] = &[("subpixel_sprite", "enable dual_source_blending;")];

/// Assembles the module called `name`.
///
/// Returns `None` when no module has that name.
pub fn module(name: &str) -> Option<String> {
    let parts = MODULES
        .iter()
        .find(|(module, _)| *module == name)
        .map(|(_, parts)| *parts)?;

    let mut pieces: Vec<&str> = PRELUDES
        .iter()
        .filter(|(module, _)| *module == name)
        .map(|(_, text)| *text)
        .collect();
    pieces.extend(parts.iter().map(|piece| part(piece)));

    Some(pieces.join("\n"))
}
