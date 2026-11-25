//! Paths, externals, backdrops and groups: the frame kinds drawn one at a time.

use iceblink::{
    BlendMode, FRAME_NO_CLIP, FrameBackdrop, FrameExternal, FrameFilter, FrameGroup, FramePath,
};

use crate::capture::mirror::Mirror;
use crate::capture::text::number::{float, list, rect};
use crate::capture::transcript::instance::device;
use crate::capture::transcript::{paint, path as geometry, space};

/// Renders a path with its geometry in path space.
pub(crate) fn path(mirror: &Mirror, path: &FramePath) -> String {
    let mut line = format!("path ink={}", device(path.ink));
    if let Some(fill) = &path.fill {
        line.push_str(&format!(
            " fill={} rule={}",
            paint::reference(mirror, fill.paint),
            geometry::rule(fill.rule)
        ));
    }
    if let Some(stroke) = &path.stroke {
        line.push_str(&format!(
            " stroke={}",
            paint::reference(mirror, stroke.paint)
        ));
        line.push_str(&geometry::stroke(&stroke.style));
    }
    if let Some(transform) = geometry::affine(path.transform) {
        line.push_str(&format!(" transform={transform}"));
    }
    if let Some(brush) = geometry::affine(path.brush) {
        line.push_str(&format!(" brush={brush}"));
    }
    for clip in path.clips.iter() {
        line.push_str(&format!(
            " inside=\"{}\" clip_rule={}",
            geometry::commands(&clip.path),
            geometry::rule(clip.rule)
        ));
    }
    line.push_str(&format!(" d=\"{}\"", geometry::commands(&path.path)));
    line
}

pub(crate) fn external(mirror: &Mirror, external: &FrameExternal) -> String {
    let quad = &external.quad;
    let mut line = format!(
        "external bounds={} texture=client{}#{}.{}",
        rect(quad.bounds),
        external.client.0,
        external.texture.slot(),
        external.texture.generation()
    );
    if quad.opacity != 1.0 {
        line.push_str(&format!(" opacity={}", float(quad.opacity)));
    }
    line.push_str(&space::suffix(mirror, quad.clip.0, quad.space.0));
    line
}

pub(crate) fn backdrop(mirror: &Mirror, backdrop: &FrameBackdrop) -> String {
    let mut line = format!(
        "backdrop bounds={} item={} filters={}",
        rect(backdrop.bounds),
        rect(backdrop.backdrop.bounds),
        filters(&backdrop.filters)
    );
    if backdrop.source != backdrop.bounds {
        line.push_str(&format!(" source={}", rect(backdrop.source)));
    }
    line.push_str(&space::suffix(
        mirror,
        backdrop.backdrop.clip.0,
        backdrop.backdrop.space.0,
    ));
    line
}

pub(crate) fn group_begin(mirror: &Mirror, index: u32, group: &FrameGroup) -> String {
    let mut line = format!("group_begin {index} bounds={}", rect(group.bounds));
    if group.source != group.bounds {
        line.push_str(&format!(" source={}", rect(group.source)));
    }
    if group.opacity != 1.0 {
        line.push_str(&format!(" opacity={}", float(group.opacity)));
    }
    if group.blend != BlendMode::NORMAL {
        line.push_str(&format!(" blend={}", blend(group.blend)));
    }
    if !group.filters.is_empty() {
        line.push_str(&format!(" filters={}", filters(&group.filters)));
    }
    if group.clip != FRAME_NO_CLIP {
        line.push_str(&format!(" clip={}", space::chain(mirror, group.clip)));
    }
    line
}

/// Renders a filter chain.
fn filters(chain: &[FrameFilter]) -> String {
    let rendered: Vec<String> = chain.iter().map(filter).collect();
    format!("[{}]", rendered.join(", "))
}

fn filter(filter: &FrameFilter) -> String {
    match filter {
        FrameFilter::Blur(deviation) => format!("blur({})", float(*deviation)),
        FrameFilter::DropShadow {
            offset,
            std_dev,
            color,
        } => format!(
            "drop_shadow({}, {}, {})",
            list(offset),
            float(*std_dev),
            paint::color(*color)
        ),
        FrameFilter::ColorMatrix(matrix) => format!("color_matrix({})", list(matrix)),
        FrameFilter::Opacity(amount) => format!("opacity({})", float(*amount)),
        FrameFilter::Shader {
            shader,
            params,
            reach,
        } => format!(
            "shader(#{shader}, params=#{params}, reach={})",
            float(*reach)
        ),
    }
}

fn blend(mode: BlendMode) -> String {
    const NAMES: [&str; 17] = [
        "normal",
        "multiply",
        "screen",
        "overlay",
        "darken",
        "lighten",
        "color_dodge",
        "color_burn",
        "hard_light",
        "soft_light",
        "difference",
        "exclusion",
        "hue",
        "saturation",
        "color",
        "luminosity",
        "plus_lighter",
    ];
    NAMES
        .get(mode.0 as usize)
        .map_or_else(|| format!("<blend {}>", mode.0), |name| (*name).to_owned())
}
