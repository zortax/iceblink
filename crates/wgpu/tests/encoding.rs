//! Blending happens on gamma-encoded values, whatever the presented image's format.
//!
//! Half-transparent grey 128 over white is 191 when blended on encoded values, and 225 when an
//! encoded attachment moves the blend into linear light.

mod common;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, plain_renderer, present, renderer};
use iceblink::Color;
use iceblink_wgpu::{SrgbTier, wgpu};

/// Returns a frame of half-transparent grey 128 over white across the whole output.
fn half_grey_over_white() -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    let white = builder.solid(opaque(255, 255, 255));
    let alpha = 128.0 / 255.0;
    let grey = builder.solid(Color([alpha * alpha, alpha * alpha, alpha * alpha, alpha]));
    let whole = rect(0.0, 0.0, SIDE as f32, SIDE as f32);
    builder.quad(filled(whole, white));
    builder.quad(filled(whole, grey));
    builder.finish()
}

#[test]
fn half_transparent_grey_over_white_blends_to_191() {
    let mut renderer = plain_renderer();

    let pixels = present(&mut renderer, &half_grey_over_white());

    assert_eq!(pixels.rgba(64, 64), [191, 191, 191, 255]);
}

#[test]
fn an_encoded_image_receives_the_composed_bytes_through_its_unencoded_view() {
    let mut renderer = renderer(wgpu::TextureFormat::Rgba8UnormSrgb);
    let formats = renderer.formats();
    assert_eq!(formats.tier, SrgbTier::ViewFormatTwin);
    assert_eq!(
        formats.scene,
        wgpu::TextureFormat::Rgba8Unorm,
        "the kept target never encodes"
    );

    let presented = present(&mut renderer, &half_grey_over_white());

    let composed = renderer.read_composed();
    assert_eq!(
        composed.rgba(64, 64),
        [191, 191, 191, 255],
        "the blend happens unencoded"
    );
    assert!(
        composed.max_difference(&presented) <= 1,
        "the copy changed the texels by {}",
        composed.max_difference(&presented)
    );
}

#[test]
fn the_kept_target_is_the_image_format_without_its_encoding() {
    for (image, kept) in [
        (
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ),
        (
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
        ),
        (
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba8Unorm,
        ),
    ] {
        let renderer = renderer(image);
        let formats = renderer.formats();
        assert_eq!(formats.surface, image);
        assert_eq!(formats.scene, kept);
        assert!(formats.is_sound());
    }
}
