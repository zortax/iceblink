//! Drawing instances of each frame kind into a surface, cut to a scissor.

mod external;
mod footprint;
mod line;
pub(crate) mod marks;
mod quad;
mod shadow;
pub(crate) mod sprite;
mod vector;

use iceblink::geom::{Device, Rect};
use iceblink::{FRAME_NO_CLIP, FramePaint};

use crate::math::text::TextGamma;
use crate::pages::Pages;
use crate::tables::{Clip, Space};
use crate::target::Surface;

pub(crate) use self::external::draw_external;
pub(crate) use self::line::draw_lines;
pub(crate) use self::quad::draw_quads;
pub(crate) use self::shadow::draw_shadows;
pub(crate) use self::vector::draw_pass;

/// The frame tables and atlas pages that instances are drawn with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scene<'a> {
    pub(crate) spaces: &'a [Space],
    pub(crate) clips: &'a [Clip],
    pub(crate) paints: &'a [FramePaint],
    pub(crate) pages: &'a Pages,
    pub(crate) gamma: TextGamma,
    /// Whether the display's stripes run blue to red.
    pub(crate) blue_to_red: bool,
}

impl Scene<'_> {
    /// Returns the space in `slot`, or the identity space for a slot past the table.
    pub(crate) fn space(&self, slot: u32) -> &Space {
        static IDENTITY: std::sync::OnceLock<Space> = std::sync::OnceLock::new();
        self.spaces
            .get(slot as usize)
            .unwrap_or_else(|| IDENTITY.get_or_init(|| Space::of(&iceblink::Matrix4::IDENTITY)))
    }

    /// Returns the clip in `slot`, or `None` for no clip.
    ///
    /// A slot past the table clips nothing.
    pub(crate) fn clip(&self, slot: u32) -> Option<&Clip> {
        if slot == FRAME_NO_CLIP {
            return None;
        }
        self.clips.get(slot as usize)
    }

    /// Returns the coverage of the device point `point` by `clip`, or full coverage without a
    /// clip.
    pub(crate) fn clip_coverage(&self, clip: Option<&Clip>, point: [f32; 2]) -> f32 {
        clip.map_or(1.0, |clip| clip.coverage(point, self.spaces, self.pages))
    }
}

/// Paints every pixel of `area` with what `shade` returns at its centre, over what `surface`
/// holds.
///
/// `row` is scratch for one row of colours.
pub(crate) fn paint_area(
    surface: &mut Surface<'_>,
    area: Rect<i32, Device>,
    row: &mut Vec<[f32; 4]>,
    shade: impl FnMut([f32; 2]) -> [f32; 4],
) {
    paint_around(surface, area, None, row, shade);
}

/// Paints `area` as [`paint_area`] does, except for the pixels of `core`.
///
/// `core` is a rectangle where `shade` returns one colour, and its pixels are painted that colour
/// without calling `shade`.
pub(crate) fn paint_around(
    surface: &mut Surface<'_>,
    area: Rect<i32, Device>,
    core: Option<(Rect<i32, Device>, [f32; 4])>,
    row: &mut Vec<[f32; 4]>,
    mut shade: impl FnMut([f32; 2]) -> [f32; 4],
) {
    let core = core.and_then(|(rect, color)| Some((rect.intersection(area)?, color)));
    for y in area.top()..area.bottom() {
        row.clear();
        let center_y = y as f32 + 0.5;
        let mut shaded = |row: &mut Vec<[f32; 4]>, columns: core::ops::Range<i32>| {
            row.extend(columns.map(|x| shade([x as f32 + 0.5, center_y])));
        };
        match core {
            Some((rect, color)) if y >= rect.top() && y < rect.bottom() => {
                shaded(row, area.left()..rect.left());
                row.extend(core::iter::repeat_n(color, rect.width() as usize));
                shaded(row, rect.right()..area.right());
            }
            _ => shaded(row, area.left()..area.right()),
        }
        surface.over_row(area.left(), y, row);
    }
}
