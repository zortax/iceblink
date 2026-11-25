//! Golden text and image comparisons.

use std::path::PathBuf;

use iceblink_testing::golden::{
    Outcome, RgbaImage, Tolerance, compare_png, compare_text, first_difference, is_blessing,
    read_png, write_png,
};

/// A path in the temporary directory, unique to this test process and `name`.
fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("iceblink-testing-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("the temporary directory is writable");
    directory.join(name)
}

#[test]
fn a_missing_golden_never_matches() {
    let path = scratch("absent.txt");
    assert_eq!(compare_text(&path, "anything\n"), Outcome::Missing);
    assert_eq!(
        compare_png(&path, &RgbaImage::new(1, 1), Tolerance::EXACT),
        Outcome::Missing
    );
}

#[test]
fn a_text_golden_matches_only_its_own_text() {
    let path = scratch("written.txt");
    std::fs::write(&path, "one\ntwo\n").expect("written");
    assert_eq!(compare_text(&path, "one\ntwo\n"), Outcome::Matched);
    assert!(matches!(
        compare_text(&path, "one\nthree\n"),
        Outcome::Differed(_)
    ));
}

#[test]
fn a_text_difference_names_the_first_differing_line() {
    let report = first_difference("a\nb\nc\n", "a\nB\nc\n").expect("they differ");
    assert!(report.contains("line 2"), "{report}");
    assert!(report.contains("- ") && report.contains("+ ") && report.contains('B'));

    let truncated = first_difference("a\nb\n", "a\n").expect("they differ");
    assert!(truncated.contains("<end of file>"), "{truncated}");
}

#[test]
fn an_image_survives_a_png_round_trip() {
    let mut image = RgbaImage::new(3, 2);
    image.set_pixel(2, 1, [10, 20, 30, 40]);
    let path = scratch("round_trip.png");
    write_png(&path, &image).expect("written");
    assert_eq!(read_png(&path).expect("read"), image);
}

#[test]
fn an_image_matches_within_its_declared_tolerance() {
    let golden = RgbaImage::new(4, 4);
    let path = scratch("tolerance.png");
    write_png(&path, &golden).expect("written");

    let mut rendering = golden.clone();
    rendering.set_pixel(1, 1, [3, 0, 0, 0]);
    rendering.set_pixel(2, 2, [1, 0, 0, 0]);
    assert!(matches!(
        compare_png(&path, &rendering, Tolerance::EXACT),
        Outcome::Differed(_)
    ));
    let one_pixel = Tolerance {
        max_channel: 1,
        max_pixels: 1,
    };
    assert_eq!(compare_png(&path, &rendering, one_pixel), Outcome::Matched);
    assert!(matches!(
        compare_png(&path, &RgbaImage::new(4, 5), one_pixel),
        Outcome::Differed(_)
    ));
}

#[test]
fn the_suite_never_runs_blessing() {
    assert!(
        !is_blessing(),
        "ICEBLINK_BLESS must not be set when the suite checks goldens"
    );
}
