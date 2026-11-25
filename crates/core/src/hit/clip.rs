//! Whether clip chains cover a device point.

use iceblink::{ClipRef, ClipShape, LocalParent, MaskMode, Matrix4, RecordedChunk};

use crate::content::ImageStore;
use crate::geometry::bounds::map;
use crate::geometry::clip::hard_edge;
use crate::hit::distance::rounded_distance;
use crate::hit::plane::unproject;
use crate::scene::Scene;
use crate::shared_content::image_key;

/// The scene, images and device point that clip coverage is read from.
pub(crate) struct Clips<'a> {
    pub(crate) scene: &'a Scene,
    pub(crate) images: &'a ImageStore,
    pub(crate) point: [f32; 2],
}

impl Clips<'_> {
    /// Reports whether the chain of clip node `slot` covers the point.
    pub(crate) fn node_chain(&self, slot: u32) -> bool {
        let mut current = Some(slot);
        let mut links = 0;
        while let Some(slot) = current {
            let Some(record) = self.scene.clips.at(slot) else {
                return true;
            };
            let Some(space) = self.scene.transforms.at(record.node.space.slot()) else {
                return false;
            };
            if !self.link(&record.node.shape, &space.world, [0.0; 2]) {
                return false;
            }

            current = record.node.parent.map(|parent| parent.slot());
            links += 1;
            if links > self.scene.clips.span() {
                return false;
            }
        }
        true
    }

    /// Reports whether the clip reference `clip` of an item of `chunk` covers the point, for a
    /// chunk drawn at `offset`.
    pub(crate) fn item(&self, chunk: &RecordedChunk, offset: [f32; 2], clip: ClipRef) -> bool {
        if clip == ClipRef::NONE {
            return true;
        }
        if clip == ClipRef::INHERIT {
            return chunk
                .props
                .clip
                .is_none_or(|clip| self.node_chain(clip.slot()));
        }
        match clip.local_index() {
            Some(index) => self.local_chain(chunk, offset, index),
            None => self.node_chain(clip.0),
        }
    }

    /// Reports whether the chunk-local clip at `index` and its parents cover the point.
    fn local_chain(&self, chunk: &RecordedChunk, offset: [f32; 2], index: u32) -> bool {
        let Some(space) = self.scene.transforms.at(chunk.props.space.slot()) else {
            return false;
        };
        let mut current = Some(index);
        while let Some(index) = current {
            let Some(local) = chunk.local_clips.get(index as usize) else {
                return true;
            };
            if !self.link(&local.shape, &space.world, offset) {
                return false;
            }

            current = match local.parent {
                // Parents are earlier clips, so the walk ends.
                LocalParent::Local(parent) if parent < index => Some(parent),
                LocalParent::Local(_) | LocalParent::None => None,
                LocalParent::Chunk => {
                    return chunk
                        .props
                        .clip
                        .is_none_or(|clip| self.node_chain(clip.slot()));
                }
                LocalParent::Node(id) => return self.node_chain(id.slot()),
            };
        }
        true
    }

    /// Reports whether one link covers the point, for a shape measured in a space with world
    /// matrix `world` and moved by `offset`.
    fn link(&self, shape: &ClipShape, world: &Matrix4, offset: [f32; 2]) -> bool {
        let moved = |rect: [f32; 4]| [rect[0] + offset[0], rect[1] + offset[1], rect[2], rect[3]];
        match shape {
            ClipShape::RoundedRect {
                rect,
                radii,
                shapes,
            } => {
                let rect = moved(*rect);

                if hard_edge(world, shape) {
                    let device = map(world, rect);
                    let [x, y] = self.point;
                    return device.x0 <= x && x <= device.x1 && device.y0 <= y && y <= device.y1;
                }

                let Some(local) = unproject(world, self.point) else {
                    return false;
                };
                // The distance underestimates far from a thin elliptical corner.
                let [x, y, width, height] = rect;
                let ramp = local[0] >= x - 0.5
                    && local[0] <= x + width + 0.5
                    && local[1] >= y - 0.5
                    && local[1] <= y + height + 0.5;
                ramp && 0.5 - rounded_distance(local, rect, *radii, *shapes) > 0.0
            }
            ClipShape::Mask {
                image,
                rect,
                source,
                mode,
            } => {
                let rect = moved(*rect);
                let Some(local) = unproject(world, self.point) else {
                    return false;
                };
                self.mask(image.slot(), rect, *source, *mode, local) > 0.0
            }
        }
    }

    /// Returns the coverage of a mask link at the local point `local`.
    fn mask(
        &self,
        slot: u32,
        rect: [f32; 4],
        source: [f32; 4],
        mode: MaskMode,
        local: [f32; 2],
    ) -> f32 {
        let [x, y, width, height] = rect;
        let inside =
            local[0] >= x && local[0] <= x + width && local[1] >= y && local[1] <= y + height;
        if !inside || width <= 0.0 || height <= 0.0 {
            return 0.0;
        }
        let key = image_key(self.scene.client, slot);
        let Some(desc) = self.images.desc(key) else {
            return 0.0;
        };

        // Texel centres lie half a texel inside their texels.
        let u = source[0] + (local[0] - x) / width * source[2] - 0.5;
        let v = source[1] + (local[1] - y) / height * source[3] - 0.5;
        let (u0, v0) = (u.floor(), v.floor());
        let (fu, fv) = (u - u0, v - v0);
        let texel = |tx: f32, ty: f32| -> f32 {
            let tx = tx.clamp(0.0, desc.width.saturating_sub(1) as f32) as u32;
            let ty = ty.clamp(0.0, desc.height.saturating_sub(1) as f32) as u32;
            let Some([r, g, b, a]) = self.images.texel(key, tx, ty) else {
                return 0.0;
            };
            let [r, g, b, a] = [r, g, b, a].map(|channel| f32::from(channel) / 255.0);
            if mode == MaskMode::LUMINANCE {
                // Premultiplied channels are the straight ones times alpha already.
                0.2126 * r + 0.7152 * g + 0.0722 * b
            } else {
                a
            }
        };
        let top = texel(u0, v0) * (1.0 - fu) + texel(u0 + 1.0, v0) * fu;
        let bottom = texel(u0, v0 + 1.0) * (1.0 - fu) + texel(u0 + 1.0, v0 + 1.0) * fu;
        top * (1.0 - fv) + bottom * fv
    }
}
