//! The scenes the coalescing rules are measured on.
//!
//! - A dashboard of twenty charts, with each legend drawn over or under its chart.
//! - A row of twelve avatars, each with its own rounded clip and nothing drawn between them.
//! - A stacked-area chart, whose bands overlap one another.
//! - Two cards whose drawings are nested to different depths, so draw order falls as the frame
//!   is emitted, and the same cards with a badge painted over the lower drawing.
//! - Drawings before, inside and after a group, none near another.
//! - A field of labels, each a background with one drawing inside it.
//! - One wide drawing, alone or with a second one in the same pass.

use iceblink::FRAME_NO_CLIP;

use crate::common::plan::rect;
use crate::common::plan::scene::Scene;

/// Returns twenty chart regions in a five-by-four grid, each clipped to its card.
///
/// With `legend_above`, each legend is drawn over its chart.
pub(crate) fn dashboard(legend_above: bool) -> Scene {
    let mut scene = Scene::new();
    scene.quad(rect(0, 0, 1920, 1080));
    for index in 0..20 {
        let x = 16 + (index % 5) * 380;
        let y = 16 + (index / 5) * 268;
        let card = rect(x, y, 360, 248);
        let plot = rect(x + 16, y + 40, 328, 192);
        let legend = rect(x + 240, y + 48, 96, 56);

        scene.quad(card);
        if !legend_above {
            scene.quad(legend);
        }
        let clip = scene.clips.only();
        scene.path(plot, clip);
        if legend_above {
            scene.quad(legend);
        }
    }
    scene
}

/// Returns twelve avatars in a row, each with its own rounded clip inside the row's clip.
pub(crate) fn avatars() -> Scene {
    let mut scene = Scene::new();
    scene.quad(rect(16, 16, 640, 64));
    let row = scene.clips.only();
    for index in 0..12 {
        let clip = scene.clips.push(row);
        scene.path(rect(24 + index * 48, 24, 48, 48), clip);
    }
    scene
}

/// Returns a stacked-area chart: five bands, each overlapping the one below it.
pub(crate) fn stacked_area() -> Scene {
    let mut scene = Scene::new();
    scene.quad(rect(16, 16, 600, 400));
    let clip = scene.clips.only();
    for band in 0..5 {
        let top = 40 + band * 40;
        scene.path(rect(32, top, 560, 400 - top), clip);
    }
    scene
}

/// Returns two cards side by side, the left nesting its drawing three backgrounds deep and the
/// right one background deep.
///
/// The right drawing comes later and takes the lower order.
pub(crate) fn falling_order() -> Scene {
    let mut scene = Scene::new();
    scene.quad(rect(0, 0, 1920, 1080));
    for inset in [0, 20, 40] {
        scene.quad(rect(
            100 + inset,
            100 + inset,
            300 - inset * 2,
            300 - inset * 2,
        ));
    }
    scene.plain_path(rect(160, 160, 180, 180));
    scene.quad(rect(700, 100, 300, 300));
    scene.plain_path(rect(760, 160, 180, 180));
    scene
}

/// Returns [`falling_order`] with a badge painted over the right drawing after both drawings.
pub(crate) fn badged_cards() -> Scene {
    let mut scene = falling_order();
    scene.quad(rect(800, 200, 60, 60));
    scene
}

/// Returns a drawing outside a group, one inside it, and one outside again.
pub(crate) fn across_a_group() -> Scene {
    let mut scene = Scene::new();
    scene.quad(rect(0, 0, 1920, 1080));
    scene.plain_path(rect(16, 16, 24, 24));
    scene.begin_group(rect(200, 16, 100, 100));
    scene.plain_path(rect(220, 40, 24, 24));
    scene.end_group();
    scene.plain_path(rect(500, 16, 24, 24));
    scene
}

/// Returns `count` labels, each a background with one drawing inside it.
pub(crate) fn label_field(count: i32) -> Scene {
    let mut scene = Scene::new();
    for index in 0..count {
        let x = (index % 160) * 12;
        let y = (index / 160) * 12;
        scene.quad(rect(x, y, 10, 10));
        scene.path(rect(x + 2, y + 2, 6, 6), FRAME_NO_CLIP);
    }
    scene
}

/// Returns one wide drawing, and a second one in the same pass when `two`.
pub(crate) fn wide(two: bool) -> Scene {
    let mut scene = Scene::new();
    scene.plain_path(rect(0, 0, 800, 400));
    if two {
        scene.plain_path(rect(150, 60, 300, 10));
    }
    scene
}
