//! The interface between atlas policy and a device.

pub mod error;
pub mod memory;

mod forward;
pub(crate) mod queue;

use iceblink::geom::{Device, Rect, Size};

use crate::atlas::texture::{TextureFormat, TextureId};

pub use crate::atlas::sink::error::SinkError;
pub use crate::atlas::sink::memory::MemorySink;

/// A device that creates, writes and destroys atlas textures.
///
/// An [`Atlas`](crate::atlas::Atlas) calls a sink with these guarantees:
///
/// - A texture is created before it is written.
/// - Every write lies inside its texture.
/// - A texture is not written after it is destroyed.
pub trait TextureSink {
    /// Creates `texture` with `size` texels of `format`.
    fn create_texture(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
    ) -> Result<(), SinkError>;

    /// Creates `texture` with `mip_levels` levels of detail, level zero with `size` texels of
    /// `format`.
    ///
    /// The default creates the texture with one level.
    fn create_texture_with_mips(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
        mip_levels: u32,
    ) -> Result<(), SinkError> {
        let _ = mip_levels;
        self.create_texture(texture, size, format)
    }

    /// Starts a group of writes that [`TextureSink::finish_uploads`] ends.
    ///
    /// The defaults of both do nothing.
    // A device sink can batch the group into shared staging memory and one submission.
    fn begin_uploads(&mut self) -> Result<(), SinkError> {
        Ok(())
    }

    /// Writes `bytes` into `bounds` of `texture`.
    ///
    /// The bytes are tightly packed rows of `format` texels, top row first.
    fn write_texture(
        &mut self,
        texture: TextureId,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError>;

    /// Writes `bytes` into `bounds` of level `mip` of `texture`.
    ///
    /// `bounds` is in the coordinates of that level. The default writes level zero with
    /// [`TextureSink::write_texture`] and drops the other levels.
    fn write_texture_mip(
        &mut self,
        texture: TextureId,
        mip: u32,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError> {
        if mip == 0 {
            return self.write_texture(texture, bounds, format, bytes);
        }
        Ok(())
    }

    /// Ends the group of writes started by [`TextureSink::begin_uploads`].
    fn finish_uploads(&mut self) {}

    /// Destroys `texture`.
    fn destroy_texture(&mut self, texture: TextureId);
}
