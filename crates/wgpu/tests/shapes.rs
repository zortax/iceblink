//! The extent of a blurred drop shadow.

mod common;

use common::build::{FrameBuilder, opaque, rect};
use common::device::{plain_renderer, present};
use iceblink::{ClipRef, FRAME_NO_CLIP, FrameShadow, ROUND, Shadow, SpaceRef};

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
