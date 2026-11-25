//! Texture atlas policy: tile allocation, holds, eviction and deferred uploads.
//!
//! [`Atlas`] packs rasterised content into the textures of four pools, one per [`TextureKind`].
//! It decides where each tile goes and which tiles are evicted, and it queues texture creations,
//! uploads and destructions. [`Atlas::flush_uploads`] performs the queued operations against a
//! [`TextureSink`], the interface to a device. [`MemorySink`] implements it over byte vectors.
//!
//! Content is cached under an [`AtlasKey`], an opaque handle paired with the kind of its pool. A
//! lookup returns an [`AtlasTile`]: the texture, the allocation and the rectangle of the content.
//!
//! ```
//! use iceblink_alloc::atlas::{Atlas, AtlasKey, AtlasLimits, MemorySink, TextureKind};
//! use iceblink::geom::Size;
//!
//! let mut sink = MemorySink::new();
//! let mut atlas = Atlas::new(AtlasLimits::default());
//!
//! let key = AtlasKey::new(0xbeef, TextureKind::Mono);
//! let tile = atlas
//!     .get_or_insert(key, Size::new(8, 8), || vec![0xff; 64])
//!     .expect("an 8x8 tile fits in a fresh atlas");
//! assert_eq!(tile.bounds.size, Size::new(8, 8));
//!
//! assert_eq!(sink.textures_created(), 0);
//! atlas.flush_uploads(&mut sink).expect("the memory sink accepts every write");
//! assert_eq!(sink.textures_created(), 1);
//! assert_eq!(sink.bytes_written(), 64);
//! ```

pub mod cache;
pub mod error;
pub mod key;
pub mod sink;
pub mod texture;
pub mod tile;

pub use crate::atlas::cache::{Atlas, AtlasLimits, AtlasReport, Eviction, UploadBytes};
pub use crate::atlas::error::AtlasError;
pub use crate::atlas::key::AtlasKey;
pub use crate::atlas::sink::{MemorySink, SinkError, TextureSink};
pub use crate::atlas::texture::{TextureFormat, TextureId, TextureKind};
pub use crate::atlas::tile::{AtlasTile, TileId};
