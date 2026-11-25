//! Values the draw cases build frames from: rectangles, damage, colours, quads, groups and
//! backdrops.

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Backdrop, BlendMode, BorderStyle, ClipRef, Color, FRAME_NO_CLIP, Filter, FrameBackdrop,
    FrameFilter, FrameGroup, PaintRef, Quad, ROUND, SpaceRef, read_extent,
};

/// Returns `[x, y, width, height]`.
pub fn rect(x: f32, y: f32, width: f32, height: f32) -> [f32; 4] {
    [x, y, width, height]
}

/// Returns the damage of the one rectangle `[x, y, width, height]`.
pub fn area(x: i32, y: i32, width: i32, height: i32) -> DamageSet {
    let mut set = DamageSet::new();
    set.absorb(Rect::new(Point::new(x, y), Size::new(width, height)));
    set
}

/// Returns the colour of eight-bit straight channels, premultiplied.
pub fn straight(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
    let [r, g, b, a] = [red, green, blue, alpha].map(|channel| f32::from(channel) / 255.0);
    Color::from_straight(r, g, b, a)
}

/// Returns an opaque colour from eight-bit channels.
pub fn opaque(red: u8, green: u8, blue: u8) -> Color {
    Color([
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        1.0,
    ])
}

/// Returns a quad with square corners over `bounds`, filled with `fill`, with no border and no
/// clip, in space slot 0.
pub fn filled(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// Returns an unclipped source-over group with `opacity` and `filters`, over `bounds` grown by
/// what the filters read.
///
/// # Panics
///
/// Panics on a shader filter.
pub fn group(bounds: [f32; 4], opacity: f32, filters: &[Filter]) -> FrameGroup {
    FrameGroup {
        bounds: read_extent(bounds, filters),
        source: read_extent(bounds, filters),
        clip: FRAME_NO_CLIP,
        opacity,
        blend: BlendMode::NORMAL,
        filters: filters.iter().map(frame_filter).collect(),
    }
}

/// Returns a backdrop over `bounds` that blurs what is beneath it by `deviation`.
pub fn frosted(bounds: [f32; 4], deviation: f32) -> FrameBackdrop {
    let filters = [Filter::Blur(deviation)];
    FrameBackdrop {
        backdrop: Backdrop {
            bounds,
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds,
        source: read_extent(bounds, &filters),
        filters: filters.iter().map(frame_filter).collect(),
    }
}

fn frame_filter(filter: &Filter) -> FrameFilter {
    match *filter {
        Filter::Blur(std_dev) => FrameFilter::Blur(std_dev),
        Filter::DropShadow {
            offset,
            std_dev,
            color,
        } => FrameFilter::DropShadow {
            offset,
            std_dev,
            color,
        },
        Filter::ColorMatrix(matrix) => FrameFilter::ColorMatrix(matrix),
        Filter::Opacity(opacity) => FrameFilter::Opacity(opacity),
        Filter::Shader { .. } => panic!("a shader filter names a slot the builder cannot know"),
    }
}
