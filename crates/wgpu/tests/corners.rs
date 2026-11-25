//! Corner exponents on quads, shadows and clips, fractional edges and rings.
//!
//! An exponent of 2 is the ellipse, 4 a squircle, 1 a straight bevel and 0.5 a scoop cut inwards.

mod common;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{plain_renderer, present};
use iceblink::{
    ClipEntry, ClipLinkEntry, ClipRef, CornerShapes, FRAME_NO_CLIP, FrameClipShape, FrameShadow,
    PaintRef, ROUND, Shadow, SpaceRef,
};

const SQUIRCLE: CornerShapes = [4.0; 4];
const BEVEL: CornerShapes = [1.0; 4];
const SCOOP: CornerShapes = [0.5; 4];

/// Returns a frame of a black 60 by 60 box at the origin with corner `shapes` and `radius`.
fn boxed(shapes: CornerShapes, radius: f32) -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    let black = builder.solid(opaque(0, 0, 0));
    let mut quad = filled(rect(0.0, 0.0, 60.0, 60.0), black);
    quad.radii = [radius; 8];
    quad.shapes = shapes;
    builder.quad(quad);
    builder.finish()
}

/// Returns the alpha at `(x, y)` of `frame` drawn by a renderer of its own.
fn alpha(frame: &iceblink::Frame, x: i32, y: i32) -> u8 {
    let mut renderer = plain_renderer();
    present(&mut renderer, frame).rgba(x, y)[3]
}

#[test]
fn a_bevel_cuts_the_corner_straight_and_a_scoop_cuts_past_it() {
    let mut renderer = plain_renderer();

    let bevel = present(&mut renderer, &boxed(BEVEL, 24.0));
    let round = present(&mut renderer, &boxed(ROUND, 24.0));
    let scoop = present(&mut renderer, &boxed(SCOOP, 24.0));

    assert_eq!(bevel.rgba(20, 20)[3], 255);
    assert_eq!(bevel.rgba(4, 4)[3], 0);
    assert!(
        round.rgba(9, 9)[3] > bevel.rgba(9, 9)[3],
        "the ellipse is fuller than the bevel"
    );
    assert!(
        bevel.rgba(16, 16)[3] > scoop.rgba(16, 16)[3],
        "the scoop cuts past the bevel"
    );
}

#[test]
fn a_square_box_is_the_same_box_whatever_exponent_it_names() {
    let mut renderer = plain_renderer();
    let plain = present(&mut renderer, &boxed(ROUND, 0.0));
    for shapes in [SQUIRCLE, BEVEL, SCOOP] {
        let named = present(&mut renderer, &boxed(shapes, 0.0));
        assert_eq!(
            named.max_difference(&plain),
            0,
            "{shapes:?} has no corner to cut"
        );
    }
}

#[test]
fn a_shadow_is_cut_by_the_exponent_of_its_caster() {
    let shadowed = |shapes: CornerShapes| {
        let mut builder = FrameBuilder::new();
        builder.shadow(FrameShadow {
            bounds: rect(8.0, 8.0, 64.0, 64.0),
            shadow: Shadow {
                shape_bounds: rect(12.0, 12.0, 56.0, 56.0),
                radii: [24.0; 8],
                caster_bounds: rect(20.0, 20.0, 40.0, 40.0),
                caster_radii: [16.0; 8],
                shapes,
                color: opaque(0, 0, 0),
                blur: 0.0,
                clip: ClipRef(FRAME_NO_CLIP),
                space: SpaceRef(0),
                inset: 0,
            },
        });
        builder.finish()
    };

    let round = shadowed(ROUND);
    let squircle = shadowed(SQUIRCLE);

    assert_eq!(
        alpha(&round, 16, 16),
        0,
        "the ellipse cuts this corner away"
    );
    assert_eq!(
        alpha(&squircle, 16, 16),
        255,
        "the squircle's shadow fills it"
    );
    assert_eq!(alpha(&round, 14, 40), 255, "the spread is drawn either way");
}

#[test]
fn a_clip_is_cut_by_the_exponent_of_its_link() {
    let clipped = |shapes: CornerShapes| {
        let mut builder = FrameBuilder::new();
        let black = builder.solid(opaque(0, 0, 0));
        let clip = builder.clip(ClipEntry {
            links: [ClipLinkEntry {
                space: 0,
                shape: FrameClipShape::RoundedRect {
                    rect: rect(0.0, 0.0, 60.0, 60.0),
                    radii: [24.0; 8],
                    shapes,
                },
            }]
            .into_iter()
            .collect(),
        });
        let mut quad = filled(rect(0.0, 0.0, 60.0, 60.0), black);
        quad.clip = clip;
        builder.quad(quad);
        builder.finish()
    };

    assert_eq!(
        alpha(&clipped(ROUND), 5, 5),
        0,
        "the ellipse clips this away"
    );
    assert_eq!(
        alpha(&clipped(SQUIRCLE), 5, 5),
        255,
        "the squircle keeps it"
    );
}

#[test]
fn an_edge_at_a_fraction_of_a_pixel_covers_that_fraction() {
    let mut renderer = plain_renderer();
    for radius in [0.0, 8.0] {
        let mut builder = FrameBuilder::new();
        let black = builder.solid(opaque(0, 0, 0));
        let mut quad = filled(rect(10.25, 10.0, 40.0, 40.0), black);
        quad.radii = [radius; 8];
        builder.quad(quad);
        let coverage = present(&mut renderer, &builder.finish()).rgba(10, 30)[3];
        assert!(
            coverage.abs_diff(191) <= 2,
            "radius {radius}: the edge holds {coverage}"
        );
    }
}

#[test]
fn a_ring_antialiases_its_inner_edge_beside_the_straight_band() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let black = builder.solid(opaque(0, 0, 0));
    let mut ring = filled(rect(11.5, 11.6, 38.0, 38.0), PaintRef::NONE);
    ring.radii = [19.0; 8];
    ring.border = [2.0; 4];
    ring.stroke = black;
    builder.quad(ring);

    let coverage = present(&mut renderer, &builder.finish()).rgba(30, 13)[3];

    assert!(
        coverage.abs_diff(153) <= 3,
        "the pixel across the inner edge holds {coverage}"
    );
}
