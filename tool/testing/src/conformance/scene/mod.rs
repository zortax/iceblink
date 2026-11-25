//! The scene suite: commits driven through any `Compositor`, and the spec's rules checked on the
//! frames that come out.
//!
//! Each case builds its compositor from the factory, registers one client, creates one opaque
//! output of 128 by 128 pixels and draws its frames through a model of a renderer. The model
//! checks every frame with the [`FrameValidator`](crate::FrameValidator), applies its notes and
//! table deltas, and resolves what it draws. A case then checks the frame against the scene it
//! committed: the damage covers what changed, overlapping items are drawn in paint order, and so
//! on. Every case gives the compositor a [`FakeGlyphRaster`](crate::fakes::FakeGlyphRaster) and a
//! [`FakeFontRegistry`](crate::fakes::FakeFontRegistry).
//!
//! The cases check the soundness rules on every compositor, and the precision rules only on one
//! that reports `CompositorCapabilities::precise`. On any other, a case reports its precision
//! checks skipped with a line on standard error. They check nothing the spec leaves to the
//! implementation, such as batching, culling of a compositor that is not precise, or which frame
//! slots are assigned.
//!
//! [`scene_suite!`](crate::scene_suite) declares one test per case, and [`run_all`] runs every
//! case in one test.

mod build;
mod cases;
mod replay;
mod stage;

use iceblink::Compositor;

use crate::conformance::Case;

pub use self::cases::colours::{item_colours, opaque_alphas};
pub use self::cases::counters::commit_counters;
pub use self::cases::coverage::damage_coverage;
pub use self::cases::damage::{
    chunk_changes, effect_writes, node_and_slot_writes, order_changes, output_changes,
    resource_writes,
};
pub use self::cases::declared::{declared_damage, hint_violations, verified_declarations};
pub use self::cases::effects::{
    clips_above_groups, effects_in_place, empty_groups_and_backdrops, group_bounds,
    isolating_effects, shader_filters_without_custom_shaders,
};
pub use self::cases::identical::identical_replace;
pub use self::cases::images::{image_edges, image_sources};
pub use self::cases::lifecycle::{
    frame_ids, idle_by_own_damage, kept_damage, minimal_renderers, pass_keys, recovery,
    shader_clock,
};
pub use self::cases::marks::projected_placements;
pub use self::cases::masks::mask_writes;
pub use self::cases::order::{paint_order, roots_of_clients, what_the_root_reaches};
pub use self::cases::paths::path_placement;
pub use self::cases::precedence::rejection_precedence;
pub use self::cases::rejections::{
    cycles, in_use, invalid_items, invalid_shaders, invalid_writes, order_conflicts, paint_kinds,
    splices_out_of_range, stale_ids, unknown_chunks, unknown_outputs, unsupported_animations,
};
pub use self::cases::replaced::replaced_objects;
pub use self::cases::routes::{idle_route_frames, path_routes, removed_path_routes, route_budget};
pub use self::cases::scroll::{anchors, scrolling, shifts};
pub use self::cases::text::{atlas_glyphs, glyph_ink, glyph_routes, glyphs_at_rest};
pub use self::cases::thumb::thumbs;

/// Builds a fresh compositor for one case.
pub type MakeCompositor<'a> = &'a dyn Fn() -> Box<dyn Compositor>;

/// Every case of the scene suite, in the order [`run_all`] runs them.
pub const CASES: &[Case<fn(MakeCompositor<'_>)>] = &[
    case("stale_ids", stale_ids),
    case("in_use", in_use),
    case("cycles", cycles),
    case("order_conflicts", order_conflicts),
    case("splices_out_of_range", splices_out_of_range),
    case("paint_kinds", paint_kinds),
    case("invalid_items", invalid_items),
    case("unknown_chunks", unknown_chunks),
    case("unknown_outputs", unknown_outputs),
    case("invalid_writes", invalid_writes),
    case("invalid_shaders", invalid_shaders),
    case("unsupported_animations", unsupported_animations),
    case("rejection_precedence", rejection_precedence),
    case("replaced_objects", replaced_objects),
    case("paint_order", paint_order),
    case("what_the_root_reaches", what_the_root_reaches),
    case("roots_of_clients", roots_of_clients),
    case("chunk_changes", chunk_changes),
    case("node_and_slot_writes", node_and_slot_writes),
    case("order_changes", order_changes),
    case("effect_writes", effect_writes),
    case("output_changes", output_changes),
    case("resource_writes", resource_writes),
    case("mask_writes", mask_writes),
    case("identical_replace", identical_replace),
    case("isolating_effects", isolating_effects),
    case("group_bounds", group_bounds),
    case("effects_in_place", effects_in_place),
    case("clips_above_groups", clips_above_groups),
    case("empty_groups_and_backdrops", empty_groups_and_backdrops),
    case(
        "shader_filters_without_custom_shaders",
        shader_filters_without_custom_shaders,
    ),
    case("damage_coverage", damage_coverage),
    case("scrolling", scrolling),
    case("shifts", shifts),
    case("anchors", anchors),
    case("thumbs", thumbs),
    case("declared_damage", declared_damage),
    case("hint_violations", hint_violations),
    case("verified_declarations", verified_declarations),
    case("frame_ids", frame_ids),
    case("kept_damage", kept_damage),
    case("idle_by_own_damage", idle_by_own_damage),
    case("recovery", recovery),
    case("shader_clock", shader_clock),
    case("minimal_renderers", minimal_renderers),
    case("pass_keys", pass_keys),
    case("image_sources", image_sources),
    case("image_edges", image_edges),
    case("path_placement", path_placement),
    case("path_routes", path_routes),
    case("removed_path_routes", removed_path_routes),
    case("idle_route_frames", idle_route_frames),
    case("route_budget", route_budget),
    case("projected_placements", projected_placements),
    case("atlas_glyphs", atlas_glyphs),
    case("glyph_routes", glyph_routes),
    case("glyph_ink", glyph_ink),
    case("glyphs_at_rest", glyphs_at_rest),
    case("item_colours", item_colours),
    case("opaque_alphas", opaque_alphas),
    case("commit_counters", commit_counters),
];

const fn case(name: &'static str, run: fn(MakeCompositor<'_>)) -> Case<fn(MakeCompositor<'_>)> {
    Case { name, run }
}

/// Runs every case of the scene suite on compositors from `make`.
///
/// # Panics
///
/// Panics after the last case when any case failed, naming every failed case.
pub fn run_all(make: MakeCompositor<'_>) {
    crate::conformance::run_cases(CASES, |case| (case.run)(make));
}

/// Declares one test per case of the scene suite, each on a compositor from the factory
/// expression `$make`, which is evaluated once per test.
///
/// ```text
/// iceblink_testing::scene_suite!(|| -> Box<dyn iceblink::Compositor> {
///     Box::new(MyCompositor::new())
/// });
/// ```
#[macro_export]
macro_rules! scene_suite {
    ($make:expr) => {
        $crate::scene_suite!(
            @cases $make;
            stale_ids, in_use, cycles, order_conflicts, splices_out_of_range, paint_kinds,
            invalid_items, unknown_chunks, unknown_outputs, invalid_writes, invalid_shaders,
            unsupported_animations, rejection_precedence, replaced_objects, paint_order, what_the_root_reaches,
            roots_of_clients, chunk_changes, node_and_slot_writes, order_changes, effect_writes,
            output_changes, resource_writes, mask_writes, identical_replace, isolating_effects,
            group_bounds, effects_in_place, clips_above_groups, empty_groups_and_backdrops,
            shader_filters_without_custom_shaders, damage_coverage, scrolling, shifts, anchors,
            thumbs, declared_damage, hint_violations, verified_declarations, frame_ids, kept_damage,
            idle_by_own_damage, recovery, shader_clock, minimal_renderers, pass_keys, image_sources,
            image_edges, path_placement, path_routes, removed_path_routes, idle_route_frames,
            route_budget,
            projected_placements, atlas_glyphs, glyph_routes, glyph_ink, glyphs_at_rest,
            item_colours, opaque_alphas, commit_counters
        );
    };
    (@cases $make:expr; $($case:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case() {
                $crate::conformance::scene::$case(&$make);
            }
        )+
    };
}
