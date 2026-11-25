//! The page operations a cache records, and the replay of every page for a new renderer.

mod common;

use std::collections::HashMap;

use common::content::{
    FIRST_COLOR, FakeRaster, cache_with, drain, glyph, input, lower, moved, run, solid,
};
use iceblink::{GlyphFlags, PageFormat, PageOp};

#[test]
fn every_upload_follows_the_creation_of_its_page_and_lies_inside_it() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let mono = run(16.0, solid(0), GlyphFlags::default());
    let striped = run(16.0, solid(0), GlyphFlags::SUBPIXEL);
    let glyphs = [
        glyph(5, 0.0, 0.0),
        glyph(FIRST_COLOR, 8.0, 0.0),
        glyph(6, 16.0, 0.0),
    ];
    let mut subpixel = input(&striped, &glyphs);
    subpixel.subpixel = true;
    lower(&mut cache, &input(&mono, &glyphs), &moved(0.0, 0.0));
    lower(&mut cache, &subpixel, &moved(0.0, 0.0));

    let ops = drain(&mut cache);

    let mut pages: HashMap<u32, (PageFormat, [u32; 2])> = HashMap::new();
    let mut uploads = 0;
    for op in &ops {
        match op {
            PageOp::Create {
                page, format, size, ..
            } => {
                assert!(pages.insert(*page, (*format, *size)).is_none());
            }
            PageOp::Upload {
                page,
                rect,
                bytes,
                offset,
                stride,
                ..
            } => {
                let (format, size) = pages.get(page).expect("an upload into a created page");
                let texel = if *format == PageFormat::A8 { 1 } else { 4 };
                assert_eq!(*stride as i32, rect.size.width * texel);
                assert!(rect.origin.x + rect.size.width <= size[0] as i32);
                assert!(rect.origin.y + rect.size.height <= size[1] as i32);
                let end = offset
                    + (rect.size.height as usize - 1) * *stride as usize
                    + rect.size.width as usize * texel as usize;
                assert!(end <= bytes.len());
                uploads += 1;
            }
            PageOp::Destroy { .. } => panic!("nothing was evicted"),
        }
    }
    assert_eq!(
        uploads, 5,
        "two mono glyphs, two subpixel glyphs, one colour glyph"
    );
    let mut formats: Vec<u8> = pages.values().map(|(format, _)| format.0).collect();
    formats.sort_unstable();
    assert_eq!(
        formats,
        [
            PageFormat::A8.0,
            PageFormat::RGBA8_PREMULTIPLIED.0,
            PageFormat::SUBPIXEL.0
        ]
    );
    assert!(drain(&mut cache).is_empty(), "draining empties the queue");
    assert!(!cache.has_page_ops());
}

#[test]
fn the_uploads_of_one_drain_share_one_buffer() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(5, 0.0, 0.0), glyph(6, 8.0, 0.0), glyph(7, 16.0, 0.0)];
    lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));

    let ops = drain(&mut cache);

    let buffers: Vec<_> = ops
        .iter()
        .filter_map(|op| match op {
            PageOp::Upload { bytes, .. } => Some(bytes.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(buffers.len(), 3);
    assert!(
        buffers
            .iter()
            .all(|bytes| std::sync::Arc::ptr_eq(bytes, &buffers[0]))
    );
    assert_eq!(cache.stats().pages_created, 1);
    assert_eq!(cache.stats().texels_uploaded, 3 * 4 * 6);
}

/// Returns the texels of every level of every page, as a renderer applying `ops` in order holds
/// them.
fn held(ops: &[PageOp]) -> std::collections::BTreeMap<(u32, u32), Vec<u8>> {
    let mut pages = std::collections::BTreeMap::new();
    let mut sizes = HashMap::new();
    for op in ops {
        match op {
            PageOp::Create {
                page,
                format,
                size,
                levels,
            } => {
                let texel = if *format == PageFormat::A8 { 1 } else { 4 };
                for level in 0..*levels {
                    let [width, height] = PageOp::level_size(*size, level);
                    sizes.insert((*page, level), (width as usize, texel));
                    pages.insert((*page, level), vec![0u8; (width * height) as usize * texel]);
                }
            }
            PageOp::Upload {
                page,
                level,
                rect,
                bytes,
                offset,
                stride,
            } => {
                let (width, texel) = sizes[&(*page, *level)];
                let texels = pages.get_mut(&(*page, *level)).expect("a created level");
                let row = rect.size.width as usize * texel;
                for line in 0..rect.size.height as usize {
                    let from = offset + line * *stride as usize;
                    let to =
                        ((rect.origin.y as usize + line) * width + rect.origin.x as usize) * texel;
                    texels[to..to + row].copy_from_slice(&bytes[from..from + row]);
                }
            }
            PageOp::Destroy { page } => pages.retain(|(held, _), _| held != page),
        }
    }
    pages
}

#[test]
fn a_replay_gives_a_renderer_every_page_without_rasterising_again() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let mono = run(16.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(5, 0.0, 0.0), glyph(FIRST_COLOR, 8.0, 0.0)];
    lower(&mut cache, &input(&mono, &glyphs), &moved(0.0, 0.0));
    let mut images = iceblink_core::content::ImageStore::new();
    let key = iceblink_core::content::ImageKey(3);
    let desc = iceblink::ImageDesc {
        width: 600,
        height: 8,
        format: iceblink::ImageFormat::RGBA8_PREMULTIPLIED,
    };
    images.create(key, desc).unwrap();
    let texels: Vec<u8> = (0..600 * 8 * 4).map(|at| (at % 251) as u8).collect();
    let write = iceblink::ImageWrite {
        rect: iceblink::geom::Rect::new(
            iceblink::geom::Point::new(0, 0),
            iceblink::geom::Size::new(600, 8),
        ),
        bytes: std::sync::Arc::from(texels),
        offset: 0,
        stride: 600 * 4,
    };
    images.write(key, &write).unwrap();
    cache.image_tile(&images, key).unwrap();
    let ops = drain(&mut cache);
    let rasterised = raster.count();

    let mut replayed = Vec::new();
    cache.replay_pages(&images, &mut replayed);

    assert_eq!(held(&replayed), held(&ops));
    assert_eq!(raster.count(), rasterised);
    assert!(cache.page_copy_bytes() > 0);
}
