//! The texture sink that turns atlas texture work into page operations.

use std::sync::Arc;

use iceblink::geom::{Device, Rect, Size};
use iceblink::{DeviceRect, PageOp};
use iceblink_alloc::atlas::{SinkError, TextureFormat, TextureId, TextureKind, TextureSink};

use crate::content::pages::held::HeldPages;
use crate::content::pages::{format_of, page_of};

/// The staging capacity kept between flushes.
const STAGING_KEPT: usize = 1 << 20;

/// The smallest upload that gets a buffer of its own instead of a place in the shared one.
// A copy into the shared buffer and then into its `Arc` doubles the copy of a large image.
const OWN_BUFFER: usize = 256 * 1024;

/// One staged upload, waiting for the end of its flush.
#[derive(Clone, Debug)]
struct Staged {
    page: u32,
    level: u32,
    rect: DeviceRect,
    stride: u32,
    bytes: StagedBytes,
}

/// Where a staged upload's bytes are.
#[derive(Clone, Debug)]
enum StagedBytes {
    /// At this offset of the shared buffer.
    Shared(usize),
    /// In a buffer of its own.
    Own(Arc<[u8]>),
}

/// A texture sink that records page operations in the order the atlas performs them.
///
/// The small uploads of one flush share one byte buffer, which each `Upload` addresses by offset.
/// The sink keeps a copy of the texels of every page that does not hold images, so a renderer
/// holding no page can be brought up to date without rasterising anything again.
#[derive(Debug, Default)]
pub(crate) struct PageSink {
    ops: Vec<PageOp>,
    held: HeldPages,
    staging: Vec<u8>,
    staged: Vec<Staged>,
    created: u64,
    destroyed: u64,
    uploaded_texels: u64,
}

impl PageSink {
    /// Appends an upload of `bytes` into level `level` of `page`.
    ///
    /// The atlas must have been flushed first, so the page's creation precedes the upload.
    pub(crate) fn push_upload(
        &mut self,
        page: u32,
        level: u32,
        rect: DeviceRect,
        bytes: Arc<[u8]>,
        offset: usize,
        stride: u32,
    ) {
        self.uploaded_texels += rect.size.width.max(0) as u64 * rect.size.height.max(0) as u64;
        self.held
            .write(page, level, rect, &bytes, offset, stride as usize);
        self.ops.push(PageOp::Upload {
            page,
            level,
            rect,
            bytes,
            offset,
            stride,
        });
    }

    /// Moves every recorded operation to the end of `out`, oldest first.
    pub(crate) fn drain_into(&mut self, out: &mut Vec<PageOp>) {
        out.append(&mut self.ops);
    }

    /// Appends a creation of every page that exists, followed by the texels of each page that
    /// does not hold images.
    pub(crate) fn replay(&self, out: &mut Vec<PageOp>) {
        self.held.replay(out);
    }

    /// Returns the bytes of the page copies the sink keeps.
    pub(crate) fn copied_bytes(&self) -> u64 {
        self.held.copied_bytes()
    }

    /// Reports whether no operation is recorded.
    pub(crate) fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Returns the pages created since the sink was made.
    pub(crate) fn pages_created(&self) -> u64 {
        self.created
    }

    /// Returns the pages destroyed since the sink was made.
    pub(crate) fn pages_destroyed(&self) -> u64 {
        self.destroyed
    }

    /// Returns the texels uploaded since the sink was made.
    pub(crate) fn uploaded_texels(&self) -> u64 {
        self.uploaded_texels
    }
}

impl TextureSink for PageSink {
    fn create_texture(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
    ) -> Result<(), SinkError> {
        self.create_texture_with_mips(texture, size, format, 1)
    }

    fn create_texture_with_mips(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        _format: TextureFormat,
        mip_levels: u32,
    ) -> Result<(), SinkError> {
        self.created += 1;
        let page = page_of(texture);
        let format = format_of(texture.kind);
        let size = [size.width.max(0) as u32, size.height.max(0) as u32];
        let levels = mip_levels.max(1);

        let copied = texture.kind != TextureKind::Image;
        self.held.create(page, format, size, levels, copied);
        self.ops.push(PageOp::Create {
            page,
            format,
            size,
            levels,
        });
        Ok(())
    }

    fn write_texture(
        &mut self,
        texture: TextureId,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError> {
        self.write_texture_mip(texture, 0, bounds, format, bytes)
    }

    fn write_texture_mip(
        &mut self,
        texture: TextureId,
        mip: u32,
        bounds: Rect<i32, Device>,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<(), SinkError> {
        let stride = bounds.size.width.max(0) as u32 * format.bytes_per_texel();
        self.held
            .write(page_of(texture), mip, bounds, bytes, 0, stride as usize);

        let staged = if bytes.len() >= OWN_BUFFER {
            StagedBytes::Own(Arc::from(bytes))
        } else {
            let offset = self.staging.len();
            self.staging.extend_from_slice(bytes);
            StagedBytes::Shared(offset)
        };
        self.staged.push(Staged {
            page: page_of(texture),
            level: mip,
            rect: bounds,
            stride,
            bytes: staged,
        });
        Ok(())
    }

    fn finish_uploads(&mut self) {
        if self.staged.is_empty() {
            return;
        }

        let shared: Arc<[u8]> = Arc::from(self.staging.as_slice());
        // A large image would otherwise keep its staging capacity for the life of the cache.
        if self.staging.capacity() > STAGING_KEPT {
            self.staging = Vec::new();
        } else {
            self.staging.clear();
        }

        for staged in self.staged.drain(..) {
            self.uploaded_texels +=
                staged.rect.size.width.max(0) as u64 * staged.rect.size.height.max(0) as u64;
            let (bytes, offset) = match staged.bytes {
                StagedBytes::Shared(offset) => (Arc::clone(&shared), offset),
                StagedBytes::Own(bytes) => (bytes, 0),
            };
            self.ops.push(PageOp::Upload {
                page: staged.page,
                level: staged.level,
                rect: staged.rect,
                bytes,
                offset,
                stride: staged.stride,
            });
        }
    }

    fn destroy_texture(&mut self, texture: TextureId) {
        self.destroyed += 1;
        let page = page_of(texture);
        self.held.destroy(page);
        self.ops.push(PageOp::Destroy { page });
    }
}
