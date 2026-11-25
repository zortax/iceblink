//! The image, external texture and backdrop of the scene.

use std::sync::Arc;

use iceblink::{
    Backdrop, ChunkRecorder, ClipRef, Extend, ExternalQuad, Filter, Image, ImageFilter, ImageFlags,
    ImageId, PaintRef, ROUND, SpaceRef, TextureId,
};

use super::Refs;

/// Records `image` in grey, repeated horizontally, inside the nested local clip.
pub(super) fn image(recorder: &mut ChunkRecorder<'_>, refs: &Refs, image: ImageId) {
    let slot = recorder.image_slot(image);
    recorder.image(Image {
        bounds: [50.0, 40.0, 16.0, 16.0],
        frame: [48.0, 38.0, 30.0, 30.0],
        source: [0.25, 0.0, 15.5, 16.0],
        radii: [5.0; 8],
        shapes: ROUND,
        paint: PaintRef::color([0, 100, 0, 255]),
        image: slot,
        opacity: 0.7,
        clip: refs.nested,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::GRAYSCALE,
        extend: [Extend::REPEAT, Extend::NONE],
        filter: ImageFilter::LINEAR,
    });
}

/// Records a quad that shows `texture`.
pub(super) fn external(recorder: &mut ChunkRecorder<'_>, texture: TextureId) {
    let slot = recorder.texture_slot(texture);
    recorder.external(ExternalQuad {
        bounds: [60.0, 40.0, 20.0, 20.0],
        texture: slot,
        opacity: 1.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
}

/// Records a backdrop that blurs what is drawn behind it.
pub(super) fn backdrop(recorder: &mut ChunkRecorder<'_>) {
    let chain = recorder.filters(Arc::new([Filter::Blur(4.0)]));
    recorder.backdrop(Backdrop {
        bounds: [20.0, 20.0, 40.0, 20.0],
        filters: chain,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
}
