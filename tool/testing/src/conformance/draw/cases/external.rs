//! External quads.

use iceblink::{
    ClientId, ClipRef, ExternalQuad, FRAME_NO_CLIP, FrameExternal, IdSpace, SpaceRef, TextureDesc,
};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIDE, SIZE};

/// An external quad whose texture slot holds no texture draws nothing.
pub fn external_without_texture(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let texture = IdSpace::new(ClientId(0)).mint::<TextureDesc>();
    builder.external(FrameExternal {
        quad: ExternalQuad {
            bounds: [16.0, 16.0, 64.0, 64.0],
            texture: texture.slot(),
            opacity: 1.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        client: ClientId(0),
        texture,
    });
    let image = canvas.draw(builder.finish());
    for y in 0..SIDE {
        for x in 0..SIDE {
            assert_eq!(
                image.pixel(x, y),
                [255; 4],
                "an external quad without a texture draws nothing, but ({x}, {y}) changed"
            );
        }
    }
}
