//! Atlas pages and rasterised content that glyph runs, images, mark shapes and small paths lower
//! into.
//!
//! A [`ContentCache`] holds atlas pages and lowers client items into frame instances:
//!
//! - A `GlyphRun` becomes mono, subpixel and colour sprites over glyph tiles, or paths for runs
//!   that are large, transformed or not painted with a solid colour.
//! - An `Image` becomes a colour sprite over the image's tile. Image pixels live in an
//!   [`ImageStore`], which survives every eviction.
//! - A `Marks` item becomes frame marks whose shape cells hold every quarter-pixel phase, and
//!   paths for the placements of shapes too large for a cell.
//! - A small solid `PathItem` becomes mono sprites over coverage masks, and any other path a
//!   frame path.
//!
//! [`ContentCache::drain_page_ops`] returns every page creation, upload and destruction in the
//! order the renderer applies them. [`ContentCache::replay_pages`] recreates every page that
//! exists for a renderer that holds none. Each lowering appends the [`TileKey`]s of the tiles it
//! read, and a tile stays resident while it is pinned.
//!
//! ```
//! use std::sync::Arc;
//!
//! use iceblink_core::content::{
//!     ContentCache, ContentConfig, ImageKey, ImageStore, Lowered, Lowering, Target,
//! };
//! use iceblink::{
//!     ClipRef, Extend, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat, ImageWrite, Matrix4,
//!     PageOp, PaintRef, ROUND, SpaceRef,
//! };
//! use iceblink::geom::{Point, Rect, Size};
//!
//! let mut images = ImageStore::new();
//! let key = ImageKey(1);
//! let desc = ImageDesc { width: 2, height: 2, format: ImageFormat::RGBA8_PREMULTIPLIED };
//! images.create(key, desc).unwrap();
//! images
//!     .write(key, &ImageWrite {
//!         rect: Rect::new(Point::new(0, 0), Size::new(2, 2)),
//!         bytes: Arc::from(vec![255u8; 16]),
//!         offset: 0,
//!         stride: 8,
//!     })
//!     .unwrap();
//!
//! let item = Image {
//!     bounds: [0.0, 0.0, 20.0, 20.0],
//!     frame: [0.0, 0.0, 20.0, 20.0],
//!     source: [0.0, 0.0, 2.0, 2.0],
//!     radii: [0.0; 8],
//!     shapes: ROUND,
//!     paint: PaintRef::NONE,
//!     image: 0,
//!     opacity: 1.0,
//!     clip: ClipRef::INHERIT,
//!     space: SpaceRef::INHERIT,
//!     flags: ImageFlags::default(),
//!     extend: [Extend::NONE; 2],
//!     filter: ImageFilter::LINEAR,
//! };
//!
//! let mut cache = ContentCache::new(ContentConfig::default());
//! cache.begin_frame();
//! let mut lowering = Lowering::default();
//! cache
//!     .lower_image(&images, key, &item, &Target::new(Matrix4::IDENTITY), &mut lowering)
//!     .unwrap();
//! assert!(matches!(lowering.items[0], Lowered::Color(_)));
//! cache.pin(&lowering.tiles);
//!
//! let mut ops = Vec::new();
//! cache.drain_page_ops(&mut ops);
//! assert!(matches!(ops[0], PageOp::Create { .. }));
//! assert!(matches!(ops[1], PageOp::Upload { .. }));
//! ```

mod cache;
mod config;
mod coverage;
mod error;
mod glyph;
mod image;
mod input;
mod key;
mod lowering;
mod marks;
mod mask;
mod memory;
mod pages;
mod path;
mod stats;
mod target;
mod tiles;

pub use crate::content::cache::ContentCache;
pub use crate::content::config::ContentConfig;
pub use crate::content::error::ContentError;
pub use crate::content::glyph::route::{
    ATLAS_MAX_LOGICAL_SIZE, ATLAS_MAX_SIZE, ATLAS_MAX_STRETCH, stretches, tiles_on_grid,
};
pub use crate::content::image::{ImageError, ImageKey, ImageStore, ImageTileRef};
pub use crate::content::input::{GlyphRunInput, RunJob};
pub use crate::content::key::TileKey;
pub use crate::content::lowering::{Lowered, Lowering};
pub use crate::content::marks::LoweredMarks;
pub use crate::content::mask::{PathOwner, PathRoute};
pub use crate::content::memory::ContentMemory;
pub use crate::content::stats::ContentStats;
pub use crate::content::target::{Target, keeps_pixels};
