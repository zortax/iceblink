//! The draw suite: fixed frames drawn by any `Renderer` and compared with golden images.
//!
//! Every case builds its frames by hand with a [`FrameBuilder`], draws them through a
//! [`DrawTarget`] from the factory, and compares the presented image with a golden PNG in
//! `tool/testing/goldens/draw`. The goldens are drawn by iceblink-wgpu on hardware, and each
//! case declares how far another renderer may differ: the largest channel difference a pixel may
//! have, and how many pixels may differ by more. A case whose feature the renderer's capabilities
//! rule out is skipped with a line on standard error.
//!
//! Every frame a case draws passes [`validate_frame`](crate::validate_frame) first, so a failure
//! is the renderer's.
//!
//! [`draw_suite!`](crate::draw_suite) declares one test per case, and [`run_all`] runs every case
//! in one test.

pub mod builder;
mod canvas;
mod cases;

use iceblink::Renderer;

use crate::conformance::Case;
use crate::golden::RgbaImage;

pub use self::builder::FrameBuilder;
pub(crate) use self::canvas::same as same_image;
pub use self::cases::blends::blend_modes;
pub use self::cases::blur_cuts::{blur_clip, blur_window};
pub use self::cases::blur_edges::blur_edges;
pub use self::cases::clips::{clip_link_boxes, clips};
pub use self::cases::colours::slot_colours;
pub use self::cases::effects::{backdrops, filters, groups};
pub use self::cases::external::external_without_texture;
pub use self::cases::lines::lines;
pub use self::cases::marks::mark_prims;
pub use self::cases::paints::gradients;
pub use self::cases::paths::{path_strokes, paths};
pub use self::cases::quads::{borders, corners};
pub use self::cases::sequence::{opaque_covers, partial_frames, resident_chunks, shifts};
pub use self::cases::shaders::shader_quads;
pub use self::cases::shadows::shadows;
pub use self::cases::sprites::{colour_sprites, coverage_sprites, subpixel_sprites};

/// A renderer under test, configured for a target of the size its factory was given.
pub trait DrawTarget {
    /// The renderer.
    fn renderer(&mut self) -> &mut dyn Renderer;

    /// Reads back the image the renderer presented last, as RGBA.
    fn presented(&mut self) -> RgbaImage;
}

/// Builds a configured renderer for a target of the given size, or returns `None` after
/// reporting why it cannot.
pub type MakeTarget<'a> = &'a dyn Fn([u32; 2]) -> Option<Box<dyn DrawTarget>>;

/// Every case of the draw suite, in the order [`run_all`] runs them.
pub const CASES: &[Case<fn(MakeTarget<'_>)>] = &[
    case("corners", corners),
    case("borders", borders),
    case("gradients", gradients),
    case("lines", lines),
    case("shadows", shadows),
    case("mark_prims", mark_prims),
    case("paths", paths),
    case("path_strokes", path_strokes),
    case("shader_quads", shader_quads),
    case("coverage_sprites", coverage_sprites),
    case("subpixel_sprites", subpixel_sprites),
    case("colour_sprites", colour_sprites),
    case("external_without_texture", external_without_texture),
    case("clips", clips),
    case("clip_link_boxes", clip_link_boxes),
    case("groups", groups),
    case("filters", filters),
    case("blur_edges", blur_edges),
    case("blur_window", blur_window),
    case("blur_clip", blur_clip),
    case("backdrops", backdrops),
    case("blend_modes", blend_modes),
    case("shifts", shifts),
    case("partial_frames", partial_frames),
    case("resident_chunks", resident_chunks),
    case("opaque_covers", opaque_covers),
    case("slot_colours", slot_colours),
];

const fn case(name: &'static str, run: fn(MakeTarget<'_>)) -> Case<fn(MakeTarget<'_>)> {
    Case { name, run }
}

/// Runs every case of the draw suite on targets from `make`.
///
/// # Panics
///
/// Panics after the last case when any case failed, naming every failed case.
pub fn run_all(make: MakeTarget<'_>) {
    crate::conformance::run_cases(CASES, |case| (case.run)(make));
}

/// Declares one test per case of the draw suite, each on targets from the factory expression
/// `$make`, which is evaluated once per test.
///
/// ```text
/// iceblink_testing::draw_suite!(|size: [u32; 2]| my_target(size));
/// ```
#[macro_export]
macro_rules! draw_suite {
    ($make:expr) => {
        $crate::draw_suite!(
            @cases $make;
            corners, borders, gradients, lines, shadows, mark_prims, paths, path_strokes,
            shader_quads, coverage_sprites, subpixel_sprites, colour_sprites,
            external_without_texture, clips, clip_link_boxes, groups, filters, blur_edges,
            blur_window, blur_clip, backdrops, blend_modes, shifts, partial_frames,
            resident_chunks, opaque_covers, slot_colours
        );
    };
    (@cases $make:expr; $($case:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case() {
                $crate::conformance::draw::$case(&$make);
            }
        )+
    };
}
