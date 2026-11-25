//! Moving kept pixels by a shift before the frame redraws its damage.

use iceblink::Shift;

use crate::target::Pixmap;

/// Moves the pixels of `shift` within `kept`, as if through a temporary, so source and
/// destination may overlap.
///
/// `scratch` holds the moved pixels in between.
pub(super) fn apply(kept: &mut Pixmap, shift: Shift, scratch: &mut Vec<[u8; 4]>) {
    let Some(source) = shift.source() else {
        return;
    };
    let width = kept.width() as i32;
    let height = kept.height() as i32;
    let left = source.left().max(0).max(-shift.by[0]);
    let top = source.top().max(0).max(-shift.by[1]);
    let right = source.right().min(width).min(width - shift.by[0]);
    let bottom = source.bottom().min(height).min(height - shift.by[1]);
    if right <= left || bottom <= top {
        return;
    }
    let columns = (right - left) as usize;
    let texels = kept.texels_mut();
    scratch.clear();
    for y in top..bottom {
        let at = (y * width + left) as usize;
        scratch.extend_from_slice(&texels[at..at + columns]);
    }
    for (row, y) in (top..bottom).enumerate() {
        let at = ((y + shift.by[1]) * width + left + shift.by[0]) as usize;
        texels[at..at + columns].copy_from_slice(&scratch[row * columns..(row + 1) * columns]);
    }
}
