//! Reading a renderer's composed target and presented image back.

use std::ops::Range;

use iceblink::geom::Size;

use crate::renderer::WgpuRenderer;
use crate::renderer::readback::{self, Pixels};

impl WgpuRenderer {
    /// Reads back the target frames are composed into.
    pub fn read_composed(&self) -> Pixels {
        readback::read(
            &self.gpu,
            self.kept.texture(),
            self.kept.format(),
            self.kept.used().size,
        )
    }

    /// Reads back the image presented last, when the presenter keeps its images.
    pub fn read_presented(&self) -> Option<Pixels> {
        let texture = self.presenter.image_texture(self.presented?)?;
        let [width, height] = self.presenter.extent();
        let size = Size::new(width as i32, height as i32);
        Some(readback::read(&self.gpu, texture, texture.format(), size))
    }

    /// Reads the rows `bands` of the image presented last into `into`, or every row when `into`
    /// holds another extent or channel order.
    ///
    /// Returns whether an image was read, which needs an image presented and kept by the
    /// presenter.
    pub fn read_presented_bands(&mut self, into: &mut Pixels, bands: &[Range<u32>]) -> bool {
        let Some(texture) = self
            .presented
            .and_then(|image| self.presenter.image_texture(image))
        else {
            return false;
        };
        let format = texture.format();
        let [width, height] = self.presenter.extent();
        let size = Size::new(width as i32, height as i32);
        let bgra = matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let whole = crate::frame::damage::every_row(size.height.max(0) as u32);
        let bands = if into.holds(size, bgra) {
            bands
        } else {
            &whole
        };

        readback::read_bands(
            &self.gpu,
            texture,
            format,
            size,
            bands,
            &mut self.staging,
            into,
        );
        true
    }
}
