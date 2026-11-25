//! One drawn instance, on one line.
//!
//! Fields at their default value are left out, and every field that changes what is drawn is
//! written.

mod boxes;
mod marks;
mod objects;
mod sprites;

use iceblink::{AtlasTile, DeviceRect, Frame, Instance, ItemSource, Lane, LoweredChunk};

use crate::capture::mirror::Mirror;

pub(crate) use self::boxes::{line, quad, shader_quad, shadow};
pub(crate) use self::marks::marks;
pub(crate) use self::objects::{backdrop, external, group_begin, path};
pub(crate) use self::sprites::{color, coverage};

/// Renders a device rectangle of whole pixels.
pub(crate) fn device(rect: DeviceRect) -> String {
    format!(
        "rect({}, {}, {}, {})",
        rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
    )
}

/// Renders an atlas tile as its page and texel rectangle.
pub(crate) fn tile(tile: AtlasTile) -> String {
    let [x, y, width, height] = tile.rect;
    format!("page{}[{x}, {y}, {width}, {height}]", tile.page)
}

/// Resolves `source` of `lane` to its instance at its final position.
///
/// Returns a description of what is missing when the frame or the mirror lacks the instance.
pub(crate) fn resolve<T: Instance>(
    frame: &Frame,
    mirror: &Mirror,
    lane: &Lane<T>,
    source: ItemSource,
    array: fn(&LoweredChunk) -> &Vec<T>,
) -> Result<T, String> {
    if source.is_transient() {
        return lane
            .items
            .get(source.index as usize)
            .copied()
            .ok_or_else(|| format!("<missing item {}>", source.index));
    }

    let Some(drawn) = frame.drawn.get(source.chunk as usize) else {
        return Err(format!("<missing drawn chunk {}>", source.chunk));
    };
    let Some(chunk) = mirror.resident.get(&drawn.revision) else {
        return Err(format!("<missing revision {}>", drawn.revision));
    };
    let Some(instance) = array(chunk).get(source.index as usize) else {
        return Err(format!(
            "<missing instance {} of revision {}>",
            source.index, drawn.revision
        ));
    };
    Ok(instance.moved(drawn.offset))
}
