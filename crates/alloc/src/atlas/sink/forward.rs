//! Sinks behind a mutable reference.

use iceblink::geom::{Device, Rect, Size};

use crate::atlas::sink::{SinkError, TextureSink};
use crate::atlas::texture::{TextureFormat, TextureId};

impl<S: TextureSink + ?Sized> TextureSink for &mut S {
    fn create_texture(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
    ) -> Result<(), SinkError> {
        (**self).create_texture(texture, size, format)
    }

    fn create_texture_with_mips(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
        mip_levels: u32,
    ) -> Result<(), SinkError> {
        (**self).create_texture_with_mips(texture, size, format, mip_levels)
    }

    fn begin_uploads(&mut self) -> Result<(), SinkError> {
        (**self).begin_uploads()
    }

    fn write_texture(
        &mut self,
        texture: TextureId,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError> {
        (**self).write_texture(texture, bounds, format, bytes)
    }

    fn write_texture_mip(
        &mut self,
        texture: TextureId,
        mip: u32,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError> {
        (**self).write_texture_mip(texture, mip, bounds, format, bytes)
    }

    fn finish_uploads(&mut self) {
        (**self).finish_uploads();
    }

    fn destroy_texture(&mut self, texture: TextureId) {
        (**self).destroy_texture(texture);
    }
}
