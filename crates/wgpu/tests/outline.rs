//! Clip links as outlines.

use iceblink::smallvec::smallvec;
use iceblink::{ClipLinkEntry, FrameClipShape, Matrix4, ROUND};
use iceblink_wgpu::vector::ClipOutline;
use iceblink_wgpu::vector::outline::rounded_rect;
use kurbo::Shape as _;

fn area(radii: [f32; 8], shapes: [f32; 4]) -> f64 {
    rounded_rect([0.0, 0.0, 300.0, 100.0], radii, shapes)
        .area()
        .abs()
}

#[test]
fn a_square_cornered_rectangle_is_its_own_outline() {
    let path = rounded_rect([0.0, 0.0, 40.0, 20.0], [0.0; 8], ROUND);

    assert_eq!(path.bounding_box(), kurbo::Rect::new(0.0, 0.0, 40.0, 20.0));
    assert!((path.area().abs() - 800.0).abs() < 1e-6);
}

#[test]
fn elliptical_corners_remove_four_quarter_ellipses() {
    let radii = [80.0, 20.0, 80.0, 20.0, 80.0, 20.0, 80.0, 20.0];
    let expected = 300.0 * 100.0 - (4.0 - std::f64::consts::PI) * 80.0 * 20.0;

    let measured = area(radii, ROUND);

    assert!((measured - expected).abs() / expected < 1e-4, "{measured}");
}

#[test]
fn radii_that_overflow_their_box_are_shrunk_uniformly() {
    let path = rounded_rect([0.0, 0.0, 100.0, 100.0], [90.0; 8], ROUND);
    let circle = std::f64::consts::PI * 50.0 * 50.0;

    assert!((path.area().abs() - circle).abs() / circle < 1e-3);
}

#[test]
fn a_squircle_corner_covers_more_than_an_elliptical_one_and_a_bevel_less() {
    let radii = [40.0; 8];

    let ellipse = area(radii, ROUND);
    let squircle = area(radii, [4.0; 4]);
    let bevel = area(radii, [1.0; 4]);
    let bevelled = 300.0 * 100.0 - 4.0 * 0.5 * 40.0 * 40.0;

    assert!(squircle > ellipse + 1.0);
    assert!(bevel < ellipse - 1.0);
    assert!((bevel - bevelled).abs() < 1e-3, "{bevel}");
}

#[test]
fn a_link_keeps_its_space_under_a_plane_matrix() {
    let link = ClipLinkEntry {
        space: 1,
        shape: FrameClipShape::RoundedRect {
            rect: [0.0, 0.0, 10.0, 10.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    };
    let spaces = [Matrix4::IDENTITY, Matrix4::translation(5.0, 7.0, 0.0)];

    let outline = ClipOutline::of(&link, &spaces).expect("a rounded rectangle has an outline");

    assert_eq!(outline.transform, kurbo::Affine::translate((5.0, 7.0)));
    assert_eq!(
        (outline.transform * outline.path).bounding_box(),
        kurbo::Rect::new(5.0, 7.0, 15.0, 17.0)
    );
}

#[test]
fn a_link_under_perspective_is_mapped_onto_the_device() {
    let link = ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::RoundedRect {
            rect: [-10.0, -10.0, 20.0, 20.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    };
    let spaces = [Matrix4::translation(0.0, 0.0, 50.0).then(&Matrix4::perspective(100.0))];

    let outline = ClipOutline::of(&link, &spaces).expect("a rounded rectangle has an outline");

    assert_eq!(outline.transform, kurbo::Affine::IDENTITY);
    let width = outline.path.bounding_box().width();
    assert!(
        (width - 40.0).abs() < 0.01,
        "a square halfway to the eye doubles, to {width}"
    );
}

#[test]
fn a_mask_link_has_no_outline() {
    let link = ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::Mask {
            rect: [0.0, 0.0, 4.0, 4.0],
            tile: iceblink::AtlasTile::default(),
            mode: iceblink::MaskMode::ALPHA,
        },
    };
    let entry = iceblink::ClipEntry {
        links: smallvec![link],
    };

    assert!(ClipOutline::of(&entry.links[0], &[Matrix4::IDENTITY]).is_none());
}
