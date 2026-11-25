//! Quads with radii, exponents and borders, shadows and lines.

mod common;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{plain_renderer, present};
use iceblink::{
    BorderStyle, ClipRef, FRAME_NO_CLIP, FrameShadow, Line, LineStyle, ROUND, Shadow, SpaceRef,
};

#[test]
fn a_rounded_corner_is_cut_and_antialiased() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let green = builder.solid(opaque(0, 255, 0));
    let mut quad = filled(rect(16.0, 16.0, 64.0, 64.0), green);
    quad.radii = [16.0; 8];
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(17, 17), [0, 0, 0, 0], "outside the arc");
    assert_eq!(pixels.rgba(48, 48), [0, 255, 0, 255], "the middle");
    // The pixel the arc crosses on the diagonal is partly covered.
    let edge = pixels.rgba(20, 20)[3];
    assert!(edge > 0 && edge < 255, "the arc is smoothed: {edge}");
}

#[test]
fn a_squircle_corner_is_fuller_than_a_round_one() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let green = builder.solid(opaque(0, 255, 0));
    let mut round = filled(rect(0.0, 0.0, 60.0, 60.0), green);
    round.radii = [30.0; 8];
    let mut squircle = filled(rect(64.0, 0.0, 60.0, 60.0), green);
    squircle.radii = [30.0; 8];
    squircle.shapes = [4.0; 4];
    builder.quad(round);
    builder.quad(squircle);

    let pixels = present(&mut renderer, &builder.finish());

    // Near the top-left corners, along the diagonal.
    assert_eq!(pixels.rgba(6, 6)[3], 0);
    assert_eq!(
        pixels.rgba(70, 6)[3],
        255,
        "the squircle reaches further into its corner"
    );
}

#[test]
fn a_dashed_border_leaves_gaps() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let field = builder.solid(opaque(255, 255, 255));
    let ink = builder.solid(opaque(0, 0, 0));
    let mut quad = filled(rect(8.0, 8.0, 112.0, 112.0), field);
    quad.border = [4.0; 4];
    quad.stroke = ink;
    quad.style = BorderStyle::DASHED;
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    let row: Vec<u8> = (8..120).map(|x| pixels.rgba(x, 10)[0]).collect();
    assert!(row.iter().any(|value| *value < 30), "dashes");
    assert!(row.iter().any(|value| *value > 225), "gaps");
}

#[test]
fn a_drop_shadow_paints_around_its_caster_and_not_inside_it() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let shape = rect(40.0, 40.0, 48.0, 48.0);
    builder.shadow(FrameShadow {
        bounds: rect(28.0, 28.0, 72.0, 72.0),
        shadow: Shadow {
            shape_bounds: shape,
            radii: [0.0; 8],
            caster_bounds: shape,
            caster_radii: [0.0; 8],
            shapes: ROUND,
            color: opaque(0, 0, 0),
            blur: 4.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            inset: 0,
        },
    });

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(64, 64)[3], 0, "nothing inside the caster");
    let halo = pixels.rgba(64, 37)[3];
    assert!(halo > 0 && halo < 255, "the blur fades outside it: {halo}");
    assert_eq!(
        pixels.rgba(64, 20)[3],
        0,
        "and is gone past three deviations"
    );
}

#[test]
fn a_dotted_line_and_a_solid_line_differ() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let line = |y: f32, style: LineStyle| Line {
        bounds: rect(8.0, y, 100.0, 4.0),
        color: opaque(255, 0, 0),
        thickness: 4.0,
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style,
        _pad: [0; 3],
    };
    builder.line(line(10.0, LineStyle::SOLID));
    builder.line(line(30.0, LineStyle::DOTTED));

    let pixels = present(&mut renderer, &builder.finish());

    assert!(
        (10..100).all(|x| pixels.rgba(x, 12)[3] == 255),
        "a solid line is unbroken"
    );
    assert!(
        (10..100).any(|x| pixels.rgba(x, 32)[3] == 0),
        "a dotted line has gaps"
    );
}
