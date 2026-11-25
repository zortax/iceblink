//! Copying the rasters of keyed passes out of the inner rasteriser's targets.

use std::sync::Arc;

use iceblink::{DeviceRect, PassKey};

use super::{Keeping, Slot};
use crate::vector::frame::VectorFrame;
use crate::vector::resource::VectorTarget;

/// Copies every keyed pass of `frame`'s round that the inner rasteriser drew in the frame before
/// too into a kept raster, unless one is kept already, and returns how many it kept.
///
/// `prepared` is how many passes of the inner plan were drawn.
pub(super) fn keep(keeping: &mut Keeping, frame: &VectorFrame<'_>, prepared: usize) -> u64 {
    let mut encoder = None;
    let mut kept = 0;
    for (at, slot) in keeping.slots.iter().enumerate() {
        let Some((key, size, target, region)) = candidate(keeping, *slot, frame.round, prepared)
        else {
            continue;
        };
        let Some((source, layer)) = keeping.inner.texture(target) else {
            continue;
        };
        let extent = wgpu::Extent3d {
            width: size[0].max(0) as u32,
            height: size[1].max(0) as u32,
            depth_or_array_layers: 1,
        };
        if extent.width == 0 || extent.height == 0 {
            continue;
        }

        let encoder = encoder.get_or_insert_with(|| {
            frame
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("iceblink.vector.keep"),
                })
        });
        let texture = copy(frame.device, encoder, source, layer, region, extent);
        keeping
            .store
            .insert(key, size, texture, outlines(frame, at), keeping.cap);
        kept += 1;
    }

    if let Some(encoder) = encoder {
        frame.queue.submit([encoder.finish()]);
    }
    kept
}

/// Returns the key, region size, inner target and raster region of the pass in `slot`, if the
/// pass is worth keeping.
///
/// A pass is worth keeping when all of these hold:
///
/// 1. It has a key, and the frame before drew a pass with the same key and region size.
/// 2. The inner rasteriser drew it in `round`, among its first `prepared` passes.
/// 3. No raster is kept for its key and region size yet.
fn candidate(
    keeping: &Keeping,
    slot: Slot,
    round: usize,
    prepared: usize,
) -> Option<(PassKey, [i32; 2], VectorTarget, DeviceRect)> {
    let Slot::Drawn { index, key, size } = slot else {
        return None;
    };
    // A key drawn once and not again is not worth a copy.
    let again = keeping.seen[0].contains(&(key, size));
    if key == PassKey::NONE || !again || index >= prepared || keeping.store.holds(key, size) {
        return None;
    }

    let drawn = keeping.inner_plan.passes.get(index)?;
    if drawn.round != round || drawn.target == VectorTarget::NONE {
        return None;
    }
    Some((key, size, drawn.target, drawn.raster_region))
}

/// Records a copy of `region` of `layer` of `source` into a new texture of `extent`, and returns
/// the texture.
fn copy(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::Texture,
    layer: u32,
    region: DeviceRect,
    extent: wgpu::Extent3d,
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iceblink.vector.kept"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: source.format(),
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: source,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: region.left().max(0) as u32,
                y: region.top().max(0) as u32,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        extent,
    );
    texture
}

/// Returns the outlines of the paths of pass `at` of `frame`'s plan, with their clips.
fn outlines(frame: &VectorFrame<'_>, at: usize) -> Vec<Arc<kurbo::BezPath>> {
    frame
        .plan
        .passes
        .get(at)
        .map(|pass| frame.plan.items_of(pass))
        .unwrap_or_default()
        .iter()
        .filter_map(|item| frame.path(item))
        .flat_map(|path| {
            core::iter::once(Arc::clone(&path.path))
                .chain(path.clips.iter().map(|clip| Arc::clone(&clip.path)))
        })
        .collect()
}
