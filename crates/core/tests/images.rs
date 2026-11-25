//! Image items, image writes and image tiles.

mod common;

use std::sync::Arc;

use common::content::{drain, solid, upload_bytes};
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    ClipRef, ColorSprite, Extend, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat,
    ImageWrite, Matrix4, PageFormat, PageOp, ROUND, SpaceRef, SpriteFlags,
};
use iceblink_core::content::{
    ContentCache, ContentConfig, ContentError, ImageError, ImageKey, ImageStore, Lowered, Lowering,
    Target,
};

/// Returns a write of `bytes` into the rectangle `x, y, width, height`, rows tightly packed.
fn write(rect: [i32; 4], bytes: Vec<u8>, texel: u32) -> ImageWrite {
    ImageWrite {
        rect: Rect::new(Point::new(rect[0], rect[1]), Size::new(rect[2], rect[3])),
        bytes: Arc::from(bytes),
        offset: 0,
        stride: rect[2] as u32 * texel,
    }
}

/// Returns a store holding one premultiplied image of `width` by `height` texels of `value`.
fn store_with(width: u32, height: u32, value: u8) -> (ImageStore, ImageKey) {
    let mut images = ImageStore::new();
    let key = ImageKey(7);
    let desc = ImageDesc {
        width,
        height,
        format: ImageFormat::RGBA8_PREMULTIPLIED,
    };
    images.create(key, desc).unwrap();
    let bytes = vec![value; (width * height * 4) as usize];
    images
        .write(key, &write([0, 0, width as i32, height as i32], bytes, 4))
        .unwrap();
    (images, key)
}

/// Returns an image item drawing `source` into a 20 by 20 square.
fn item(source: [f32; 4], flags: ImageFlags) -> Image {
    Image {
        bounds: [0.0, 0.0, 20.0, 20.0],
        frame: [0.0, 0.0, 20.0, 20.0],
        source,
        radii: [2.0; 8],
        shapes: ROUND,
        paint: solid(5),
        image: 0,
        opacity: 1.5,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
        extend: [Extend::NONE; 2],
        filter: ImageFilter::LINEAR,
    }
}

/// Lowers `item` and returns its sprite.
fn sprite(
    cache: &mut ContentCache,
    images: &ImageStore,
    key: ImageKey,
    item: &Image,
) -> ColorSprite {
    let mut out = Lowering::default();
    cache
        .lower_image(images, key, item, &Target::new(Matrix4::IDENTITY), &mut out)
        .unwrap();
    match out.items.pop() {
        Some(Lowered::Color(sprite)) => sprite,
        other => panic!("expected a colour sprite, got {other:?}"),
    }
}

#[test]
fn colour_mode_draws_the_source_rectangle_of_the_image_tile() {
    let (images, key) = store_with(4, 4, 255);
    let mut cache = ContentCache::new(ContentConfig::default());
    let whole = sprite(
        &mut cache,
        &images,
        key,
        &item([0.0, 0.0, 4.0, 4.0], ImageFlags::default()),
    );
    let part = sprite(
        &mut cache,
        &images,
        key,
        &item([1.0, 2.0, 2.0, 1.0], ImageFlags::default()),
    );

    assert_eq!(whole.flags, SpriteFlags::default());
    assert_eq!(whole.opacity, 1.0, "opacity is clamped");
    assert_eq!(whole.radii, [2.0; 8]);
    assert_eq!(whole.paint, solid(5));
    assert_eq!(&whole.tile.rect[2..], &[4, 4]);
    assert_eq!(part.tile.page, whole.tile.page);
    assert_eq!(
        part.tile.rect,
        [whole.tile.rect[0] + 1, whole.tile.rect[1] + 2, 2, 1]
    );
    assert_eq!(cache.stats().image_tiles_placed, 1);
}

#[test]
fn image_flags_become_sprite_flags_and_a_mask_ignores_grayscale() {
    let (images, key) = store_with(2, 2, 255);
    let mut cache = ContentCache::new(ContentConfig::default());
    let source = [0.0, 0.0, 2.0, 2.0];

    let gray = sprite(
        &mut cache,
        &images,
        key,
        &item(source, ImageFlags::GRAYSCALE),
    );
    let mask = sprite(
        &mut cache,
        &images,
        key,
        &item(source, ImageFlags::MASK | ImageFlags::GRAYSCALE),
    );
    let mut tiled = item(source, ImageFlags::default());
    tiled.extend = [Extend::REPEAT, Extend::NONE];
    tiled.filter = ImageFilter::NEAREST;
    let tiled = sprite(&mut cache, &images, key, &tiled);

    assert_eq!(gray.flags, SpriteFlags::GRAYSCALE);
    assert_eq!(mask.flags, SpriteFlags::MASK);
    assert_eq!(tiled.flags, SpriteFlags::REPEAT_X | SpriteFlags::NEAREST);
}

#[test]
fn a_fractional_source_edge_widens_the_tile_and_the_bounds_together() {
    let (images, key) = store_with(2, 2, 255);
    let mut cache = ContentCache::new(ContentConfig::default());

    let sprite = sprite(
        &mut cache,
        &images,
        key,
        &item([0.5, 0.0, 1.0, 2.0], ImageFlags::default()),
    );

    assert_eq!(sprite.tile.rect[2], 2);
    assert_eq!(sprite.bounds, [-10.0, 0.0, 40.0, 20.0]);
    assert_eq!(sprite.frame, [0.0, 0.0, 20.0, 20.0]);
}

#[test]
fn a_widened_sprite_is_cut_to_the_item_bounds() {
    let (images, key) = store_with(2, 2, 255);
    let mut cache = ContentCache::new(ContentConfig::default());
    let mut wide = item([0.5, 0.0, 1.0, 2.0], ImageFlags::default());
    wide.frame = [-10.0, -10.0, 40.0, 40.0];
    wide.radii = [2.0, 2.0, 12.0, 12.0, 12.0, 12.0, 2.0, 2.0];

    let sprite = sprite(&mut cache, &images, key, &wide);

    assert_eq!(
        sprite.bounds,
        [-10.0, 0.0, 40.0, 20.0],
        "the bounds are widened"
    );
    assert_eq!(
        sprite.frame,
        [0.0, -10.0, 20.0, 40.0],
        "the frame is cut on x"
    );
    assert_eq!(
        sprite.radii,
        [0.0, 0.0, 12.0, 12.0, 12.0, 12.0, 0.0, 0.0],
        "only corners wholly cut lose their rounding"
    );
}

#[test]
fn straight_and_alpha_only_texels_are_stored_premultiplied() {
    let mut images = ImageStore::new();
    let straight = ImageKey(1);
    let alpha = ImageKey(2);
    let desc = |format| ImageDesc {
        width: 1,
        height: 1,
        format,
    };
    images.create(straight, desc(ImageFormat::RGBA8)).unwrap();
    images.create(alpha, desc(ImageFormat::A8)).unwrap();

    images
        .write(straight, &write([0, 0, 1, 1], vec![255, 128, 0, 128], 4))
        .unwrap();
    images
        .write(alpha, &write([0, 0, 1, 1], vec![77], 1))
        .unwrap();

    assert_eq!(images.texel(straight, 0, 0), Some([128, 64, 0, 128]));
    assert_eq!(images.texel(alpha, 0, 0), Some([0, 0, 0, 77]));
    assert_eq!(images.bytes(), 8);
}

#[test]
fn writes_outside_the_image_or_short_of_bytes_are_refused() {
    let (mut images, key) = store_with(2, 2, 0);

    assert_eq!(
        images.write(key, &write([1, 1, 2, 1], vec![0; 8], 4)),
        Err(ImageError::OutOfBounds)
    );
    assert_eq!(
        images.write(key, &write([0, 0, 2, 2], vec![0; 12], 4)),
        Err(ImageError::ShortBytes)
    );
    assert_eq!(
        images.write(ImageKey(99), &write([0, 0, 1, 1], vec![0; 4], 4)),
        Err(ImageError::UnknownImage)
    );

    let mut cache = ContentCache::new(ContentConfig::default());
    let mut out = Lowering::default();
    let unknown = cache.lower_image(
        &images,
        ImageKey(99),
        &item([0.0; 4], ImageFlags::default()),
        &Target::new(Matrix4::IDENTITY),
        &mut out,
    );
    assert_eq!(unknown, Err(ContentError::UnknownImage));
}

#[test]
fn a_large_image_gets_a_page_of_its_own_with_levels_down_to_one_texel() {
    let (images, key) = store_with(600, 10, 9);
    let mut cache = ContentCache::new(ContentConfig::default());
    let tile = cache.image_tile(&images, key).unwrap();

    let ops = drain(&mut cache);
    assert!(matches!(
        ops[0],
        PageOp::Create { page, format, size: [600, 10], levels: 10 }
            if page == tile.tile.page && format == PageFormat::RGBA8_PREMULTIPLIED
    ));
    assert_eq!(tile.tile.rect, [0, 0, 600, 10]);
    assert_eq!(upload_bytes(&ops[1]).unwrap()[0], 9);
    let levels: Vec<(u32, [i32; 2])> = ops[1..]
        .iter()
        .map(|op| match op {
            PageOp::Upload { level, rect, .. } => (*level, [rect.size.width, rect.size.height]),
            other => panic!("expected an upload, got {other:?}"),
        })
        .collect();
    assert_eq!(levels.len(), 10);
    assert_eq!(levels[1], (1, [300, 5]));
    assert_eq!(levels[9], (9, [1, 1]));
}

#[test]
fn a_write_into_an_image_with_levels_uploads_the_changed_texels_of_each_level() {
    let (mut images, key) = store_with(600, 4, 0);
    let mut cache = ContentCache::new(ContentConfig::default());
    cache.image_tile(&images, key).unwrap();
    drain(&mut cache);

    let update = write([8, 0, 4, 4], vec![255; 4 * 4 * 4], 4);
    images.write(key, &update).unwrap();
    cache.refresh_image(&images, key, &update);

    let ops = drain(&mut cache);
    let uploads: Vec<(u32, [i32; 4], u8)> = ops
        .iter()
        .map(|op| match op {
            PageOp::Upload { level, rect, .. } => (
                *level,
                [
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ],
                upload_bytes(op).unwrap()[0],
            ),
            other => panic!("expected an upload, got {other:?}"),
        })
        .collect();
    assert!(uploads.contains(&(0, [8, 0, 4, 4], 255)));
    assert!(
        uploads.contains(&(1, [4, 0, 2, 2], 255)),
        "level 1 is all written texels: {uploads:?}"
    );
    assert!(uploads.contains(&(2, [2, 0, 1, 1], 255)));
    assert!(
        uploads.contains(&(3, [1, 0, 1, 1], 128)),
        "level 3 averages written and unwritten texels"
    );
}

#[test]
fn an_image_past_the_largest_page_is_reduced_by_a_whole_factor() {
    let (images, key) = store_with(130, 6, 255);
    let config = ContentConfig {
        page_size: 64,
        max_page_size: 64,
        pages_per_pool: 4,
    };
    let mut cache = ContentCache::new(config);

    let tile = cache.image_tile(&images, key).unwrap();
    let sprite = sprite(
        &mut cache,
        &images,
        key,
        &item([0.0, 0.0, 130.0, 6.0], ImageFlags::default()),
    );

    assert_eq!(&tile.tile.rect[2..], &[44, 2]);
    assert_eq!(tile.scale, 1.0 / 3.0);
    assert_eq!(&sprite.tile.rect[2..], &[44, 2]);
}

#[test]
fn a_write_into_a_placed_image_uploads_the_written_rectangle_from_its_own_bytes() {
    let (mut images, key) = store_with(4, 4, 0);
    let mut cache = ContentCache::new(ContentConfig::default());
    let tile = cache.image_tile(&images, key).unwrap();
    drain(&mut cache);

    let update = write([1, 2, 2, 1], vec![200; 8], 4);
    images.write(key, &update).unwrap();
    cache.refresh_image(&images, key, &update);

    let ops = drain(&mut cache);
    match &ops[..] {
        [
            PageOp::Upload {
                page,
                level: 0,
                rect,
                bytes,
                offset: 0,
                stride: 8,
            },
        ] => {
            assert_eq!(*page, tile.tile.page);
            assert_eq!(
                rect.origin,
                Point::new(tile.tile.rect[0] as i32 + 1, tile.tile.rect[1] as i32 + 2)
            );
            assert_eq!(rect.size, Size::new(2, 1));
            assert!(
                Arc::ptr_eq(bytes, &update.bytes),
                "premultiplied writes upload without a copy"
            );
        }
        other => panic!("expected one upload, got {other:?}"),
    }
}

#[test]
fn a_released_image_loses_its_tile_unless_it_is_pinned() {
    let (images, key) = store_with(2, 2, 1);
    let mut cache = ContentCache::new(ContentConfig::default());
    let tile = cache.image_tile(&images, key).unwrap();
    cache.pin(&[tile.key]);
    cache.begin_frame();

    cache.release_image(&images, key);
    assert!(cache.is_resident(tile.key));

    cache.unpin(&[tile.key]);
    cache.release_image(&images, key);
    assert!(!cache.is_resident(tile.key));
}

#[test]
fn releasing_an_image_counts_as_an_eviction() {
    let (images, key) = store_with(2, 2, 1);
    let mut cache = ContentCache::new(ContentConfig::default());
    cache.image_tile(&images, key).unwrap();
    cache.begin_frame();
    let before = cache.evictions();

    cache.release_image(&images, key);

    assert_eq!(cache.evictions(), before + 1);
}
