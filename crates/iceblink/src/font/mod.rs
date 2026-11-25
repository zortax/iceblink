//! Fonts and the registry that assigns their ids.

mod registry;

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

pub use self::registry::FontRegistry;

/// A font, minted by a font registry.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct FontId(pub u32);

/// A font file, shared with whatever else holds it, such as a font database or a memory map.
pub type FontBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// A font file and the index of a face in it.
///
/// Equality and hashing compare the bytes of the file and the index.
#[derive(Clone)]
pub struct FontData {
    /// The bytes of the font file.
    pub file: FontBytes,
    /// The face's index in a collection, zero for a single face.
    pub index: u32,
}

impl FontData {
    /// Returns face `index` of `file`, which is shared without being copied.
    pub fn new(file: impl AsRef<[u8]> + Send + Sync + 'static, index: u32) -> Self {
        Self {
            file: Arc::new(file),
            index,
        }
    }

    /// Returns the bytes of the font file.
    pub fn bytes(&self) -> &[u8] {
        (*self.file).as_ref()
    }
}

impl PartialEq for FontData {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && (Arc::ptr_eq(&self.file, &other.file) || self.bytes() == other.bytes())
    }
}

impl Eq for FontData {}

impl Hash for FontData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bytes().hash(state);
        self.index.hash(state);
    }
}

impl fmt::Debug for FontData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontData")
            .field("bytes", &self.bytes().len())
            .field("index", &self.index)
            .finish()
    }
}
