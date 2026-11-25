//! The vertical slice through a capturing renderer: every story's frames as stable transcripts.

mod common;

use std::path::PathBuf;

use iceblink_testing::capture::CaptureRenderer;
use iceblink_testing::golden::assert_text;

use common::FULL;
use common::slice::Slice;
use common::stories;
use common::story::Story;

/// Plays `story` one step per frame and returns the transcripts of every frame.
fn transcripts(story: &Story) -> String {
    let renderer = CaptureRenderer::with_capabilities(FULL).keeping_all();
    let mut slice = Slice::new(renderer);
    slice.play(story);
    slice
        .renderer
        .captured()
        .iter()
        .map(|captured| captured.transcript.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the path of the golden transcripts of story `name`.
fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/slice")
        .join(format!("{name}.txt"))
}

/// Checks that `story` plays the same twice and matches its golden transcripts.
fn check(story: &Story) {
    let text = transcripts(story);
    assert_eq!(
        text,
        transcripts(story),
        "{} plays the same twice",
        story.name
    );
    assert_text(&golden(story.name), &text);
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
