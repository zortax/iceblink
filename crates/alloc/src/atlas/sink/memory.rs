//! A sink over byte vectors.

use iceblink::geom::{Device, Rect, Size};
use rustc_hash::FxHashMap;

use crate::atlas::sink::TextureSink;
use crate::atlas::sink::error::SinkError;
use crate::atlas::texture::{TextureFormat, TextureId};

/// One texture's texels.
#[derive(Clone, Debug)]
struct Stored {
    size: Size<i32, Device>,
    format: TextureFormat,
    /// Tightly packed rows, top row first.
    texels: Vec<u8>,
}

/// A [`TextureSink`] that stores textures as byte vectors and reads them back.
///
/// Textures start zeroed. Levels of detail above zero are dropped.
///
/// ```
/// use iceblink_alloc::atlas::{MemorySink, TextureFormat, TextureId, TextureKind, TextureSink};
/// use iceblink::geom::{Point, Rect, Size};
///
/// let mut sink = MemorySink::new();
/// let texture = TextureId::new(TextureKind::Mono, 0);
/// sink.create_texture(texture, Size::new(4, 4), TextureFormat::R8Unorm).unwrap();
/// sink.write_texture(
///     texture,
///     Rect::new(Point::new(1, 1), Size::new(2, 2)),
///     TextureFormat::R8Unorm,
///     &[7, 7, 7, 7],
/// )
/// .unwrap();
///
/// assert_eq!(sink.texel(texture, 1, 1), Some(&[7][..]));
/// assert_eq!(sink.texel(texture, 0, 0), Some(&[0][..]));
/// ```
#[derive(Clone, Debug, Default)]
pub struct MemorySink {
    textures: FxHashMap<TextureId, Stored>,
    bytes_written: u64,
    writes: u64,
    textures_created: u64,
}

impl MemorySink {
    /// Creates a sink with no textures.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of existing textures.
    pub fn live_textures(&self) -> usize {
        self.textures.len()
    }

    /// Returns the number of bytes written since the sink was created.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Returns the number of writes since the sink was created.
    pub fn writes(&self) -> u64 {
        self.writes
    }

    /// Returns the number of textures created since the sink was created, destroyed ones included.
    pub fn textures_created(&self) -> u64 {
        self.textures_created
    }

    /// Returns the extent of `texture`, if it exists.
    pub fn size_of(&self, texture: TextureId) -> Option<Size<i32, Device>> {
        self.textures.get(&texture).map(|stored| stored.size)
    }

    /// Returns the bytes of the texel at `x`, `y` of `texture`.
    ///
    /// Returns `None` if the texture does not exist or the texel lies outside it.
    pub fn texel(&self, texture: TextureId, x: i32, y: i32) -> Option<&[u8]> {
        let stored = self.textures.get(&texture)?;
        if x < 0 || y < 0 || x >= stored.size.width || y >= stored.size.height {
            return None;
        }
        let stride = stored.format.bytes_per_texel() as usize;
        let offset = (y as usize * stored.size.width as usize + x as usize) * stride;
        Some(&stored.texels[offset..offset + stride])
    }
}

impl TextureSink for MemorySink {
    fn create_texture(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
    ) -> Result<(), SinkError> {
        if size.width <= 0 || size.height <= 0 {
            return Err(SinkError::new(format!(
                "a texture must have a positive extent, not {}x{}",
                size.width, size.height
            )));
        }
        let bytes = format.bytes_for(size.width as u32, size.height as u32) as usize;
        self.textures_created += 1;
        self.textures.insert(
            texture,
            Stored {
                size,
                format,
                texels: vec![0; bytes],
            },
        );
        Ok(())
    }

    fn write_texture(
        &mut self,
        texture: TextureId,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError> {
        let stored = self
            .textures
            .get_mut(&texture)
            .ok_or_else(|| SinkError::new(format!("texture {texture:?} does not exist")))?;
        let stride = format.bytes_per_texel() as usize;
        let row = bounds.size.width as usize * stride;
        if bytes.len() != row * bounds.size.height as usize {
            return Err(SinkError::new(format!(
                "a {}x{} write needs {} bytes, not {}",
                bounds.size.width,
                bounds.size.height,
                row * bounds.size.height as usize,
                bytes.len()
            )));
        }

        for line in 0..bounds.size.height {
            let source = line as usize * row;
            let target = ((bounds.origin.y + line) as usize * stored.size.width as usize
                + bounds.origin.x as usize)
                * stride;
            stored.texels[target..target + row].copy_from_slice(&bytes[source..source + row]);
        }
        self.bytes_written += bytes.len() as u64;
        self.writes += 1;
        Ok(())
    }

    fn destroy_texture(&mut self, texture: TextureId) {
        self.textures.remove(&texture);
    }
}
