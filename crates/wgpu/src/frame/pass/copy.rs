//! Copies between targets that the encoder records between passes.

use iceblink::geom::{Device, Rect, Size};

use crate::frame::pass::Recorder;
use crate::frame::segment::EncoderOp;

impl Recorder<'_> {
    /// Records the copy `op`, cut to both targets.
    pub(super) fn run(&self, encoder: &mut wgpu::CommandEncoder, op: EncoderOp) {
        match op {
            EncoderOp::Capture {
                source,
                destination,
                region,
            } => {
                let Some(from) = self.texture(source) else {
                    return;
                };
                let Some(to) = self.texture(destination) else {
                    return;
                };
                // Both targets are at one resolution, so the device region is the same number of
                // texels in each. Each side measures it from its own origin.
                debug_assert_eq!(source.scale(), destination.scale());
                let scale = source.scale();
                let read = scale.texels_at(region, source.origin());
                let write = scale.texels_at(region, destination.origin());
                let (read, write) = clamp_copy(read, write, size_of(from), size_of(to));
                if read.is_empty() {
                    return;
                }
                encoder.copy_texture_to_texture(
                    texel_copy(from, read),
                    texel_copy(to, write),
                    wgpu::Extent3d {
                        width: read.size.width as u32,
                        height: read.size.height as u32,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
    }
}

/// Returns `read` and `write` cut so that both lie inside their textures, `from` and `to`.
fn clamp_copy(
    read: Rect<i32, Device>,
    write: Rect<i32, Device>,
    from: Size<i32, Device>,
    to: Size<i32, Device>,
) -> (Rect<i32, Device>, Rect<i32, Device>) {
    let bound = |size| Rect::new(iceblink::geom::Point::new(0, 0), size);
    // The destination's bounds in the source's texels, and back.
    let forward = read.origin.offset_from(write.origin);
    let back = write.origin.offset_from(read.origin);
    match read
        .intersection(bound(from))
        .and_then(|clipped| clipped.intersection(bound(to).translate(forward)))
    {
        Some(read) => (read, read.translate(back)),
        None => (Rect::ZERO, Rect::ZERO),
    }
}

fn size_of(texture: &wgpu::Texture) -> Size<i32, Device> {
    Size::new(texture.width() as i32, texture.height() as i32)
}

fn texel_copy(
    texture: &wgpu::Texture,
    region: Rect<i32, Device>,
) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d {
            x: region.origin.x.max(0) as u32,
            y: region.origin.y.max(0) as u32,
            z: 0,
        },
        aspect: wgpu::TextureAspect::All,
    }
}
