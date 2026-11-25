//! The vertical slice on a device: every story drawn by the wgpu renderer through its offscreen
//! presenter, against golden images.

mod common;

use std::path::PathBuf;

use iceblink_testing::golden::{RgbaImage, Tolerance, assert_png, difference};
use iceblink_testing::gpu::{Driver, device_lock};

use common::gpu::{presented, slice};
use common::slice::SIDE;
use common::stories;
use common::story::Story;

/// How far a device may draw from the golden its driver blessed.
const GOLDEN: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 64,
};

/// How far a frame drawn incrementally may differ from the same picture drawn whole.
const WHOLE: Tolerance = Tolerance {
    max_channel: 1,
    max_pixels: 0,
};

/// Plays `story` one step per frame and returns the last image presented, or `None` when no
/// device opens.
fn played(story: &Story) -> Option<RgbaImage> {
    let mut slice = slice()?;
    slice.draw();
    slice.play(story);
    Some(presented(&slice))
}

/// Commits `story` in one transaction and returns the image of the first frame, or `None` when no
/// device opens.
fn whole(story: &Story) -> Option<RgbaImage> {
    let mut slice = slice()?;
    slice.apply_all(story);
    assert!(slice.draw(), "the first frame is built");
    Some(presented(&slice))
}

fn golden(name: &str) -> PathBuf {
    let driver = match Driver::requested() {
        Driver::Hardware => "hardware",
        Driver::Lavapipe => "lavapipe",
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/device")
        .join(driver)
        .join(format!("{name}.png"))
}

/// Reports how many distinct colours an image holds.
fn colours(image: &RgbaImage) -> usize {
    let mut seen = image.pixels().as_chunks::<4>().0.to_vec();
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

/// Asserts that `actual` differs from `expected` by no more than `tolerance`.
fn close(
    expected: &RgbaImage,
    actual: &RgbaImage,
    tolerance: Tolerance,
    what: &str,
    story: &Story,
) {
    let found = difference(expected, actual, tolerance).expect("the images have one size");
    assert!(
        found.pixels <= tolerance.max_pixels,
        "{} does not draw {what}: {found:?}",
        story.name
    );
}

/// Draws `story` incrementally, twice, and whole, and checks the pictures against each other and
/// against the driver's golden.
fn check(story: &Story) {
    let _device = device_lock();
    let Some(drawn) = played(story) else {
        return;
    };
    assert!(
        colours(&drawn) > 1,
        "{} draws more than its background",
        story.name
    );

    let again = played(story).expect("the device opened before");
    close(&drawn, &again, Tolerance::EXACT, "the same twice", story);
    let whole = whole(story).expect("the device opened before");
    close(
        &whole,
        &drawn,
        WHOLE,
        "the same incrementally and whole",
        story,
    );
    assert_png(&golden(story.name), &drawn, GOLDEN);
}

#[test]
fn boxes_borders_gradients_and_lines() {
    check(&stories::BOXES);
}

#[test]
fn text_on_the_atlas_and_path_routes() {
    check(&stories::TEXT);
}

#[test]
fn images_and_mask_clips() {
    check(&stories::IMAGES);
}

#[test]
fn paths_as_masks_and_frame_paths() {
    check(&stories::PATHS);
}

#[test]
fn marks_with_placed_shapes() {
    check(&stories::MARKS);
}

#[test]
fn groups_and_effects() {
    check(&stories::GROUPS);
}

#[test]
fn scrolling_with_shifts() {
    check(&stories::SCROLL);
}

#[test]
fn clip_edges_and_corners_that_change() {
    check(&stories::CLIPS);
}

#[test]
fn overlapping_placements_of_a_large_shape_blend_once_under_union() {
    use std::sync::Arc;

    use iceblink::Compositor as _;
    use iceblink::kurbo::Shape as _;
    use iceblink::{
        ChunkKey, Color, MarkFlags, MarkPayload, MarkPlacement, Marks, OrderEntry, Paint, PaintKind,
    };

    let _device = device_lock();
    let Some(mut slice) = slice() else {
        return;
    };
    // A shape wider than a quarter of a page, which is drawn as a path.
    let shape = iceblink::kurbo::Rect::new(0.0, 0.0, 300.0, 300.0).to_path(0.1);
    let placement = |x: f32, y: f32| MarkPlacement {
        x,
        y,
        shape: 0,
        _pad: 0,
    };
    let payload = Arc::new(MarkPayload {
        shapes: vec![Arc::new(shape)],
        placements: vec![placement(10.0, 10.0), placement(60.0, 60.0)],
        ..MarkPayload::default()
    });

    let mut tx = slice.tx();
    let half = tx.create_paint(Paint::Solid(Color([0.5, 0.5, 0.5, 0.5])));
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, common::props(slice.world.space));
    let paint = chunk.paint_ref(PaintKind::SOLID, half);
    let index = chunk.payload(payload);
    chunk.marks(Marks {
        bounds: [0.0, 0.0, SIDE as f32, SIDE as f32],
        paint,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements: 2,
        first: 0,
        payload: index,
        clip: iceblink::ClipRef::INHERIT,
        space: iceblink::SpaceRef::INHERIT,
        flags: MarkFlags::UNION,
    });
    tx.splice(slice.world.root, 0, 0, &[OrderEntry::Chunk(key)]);
    slice.compositor.commit(tx).expect("the marks commit");
    assert!(slice.draw(), "the marks are drawn");

    let image = presented(&slice);
    let single = image.pixel(30, 30);
    let overlap = image.pixel(90, 90);
    assert!(single[0] > 100, "one placement paints {single:?}");
    for channel in 0..4 {
        assert!(
            single[channel].abs_diff(overlap[channel]) <= 1,
            "one placement paints {single:?} and the overlap {overlap:?}"
        );
    }
}
