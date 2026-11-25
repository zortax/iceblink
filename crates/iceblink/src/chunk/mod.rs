//! Chunks: ordered lists of items under one key, with default property state.

mod key;
mod local_clip;
mod props;

pub use self::key::ChunkKey;
pub use self::local_clip::{LocalClip, LocalParent};
pub use self::props::{ChunkFlags, ChunkProps};
