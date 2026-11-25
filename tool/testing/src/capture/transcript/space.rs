//! Frame clip and space slots, resolved through their tables.

use iceblink::{ClipLinkEntry, FRAME_NO_CLIP, FrameClipShape, MaskMode, Matrix4, ROUND};

use crate::capture::mirror::Mirror;
use crate::capture::text::number::{all_zero, list, rect};
use crate::capture::transcript::instance::tile;

/// Renders the clip and space an instance is drawn through.
///
/// `FRAME_NO_CLIP` and an identity space render as nothing.
pub(crate) fn suffix(mirror: &Mirror, clip: u32, space: u32) -> String {
    let mut text = String::new();
    if clip != FRAME_NO_CLIP {
        text.push_str(&format!(" clip={}", chain(mirror, clip)));
    }
    if let Some(matrix) = matrix(mirror, space) {
        text.push_str(&format!(" space={matrix}"));
    }
    text
}

/// Renders the links of a clip chain, outermost first.
pub(crate) fn chain(mirror: &Mirror, clip: u32) -> String {
    let Some(entry) = mirror.clips.get(clip as usize) else {
        return format!("#{clip} <missing>");
    };

    let links: Vec<String> = entry.links.iter().map(|held| link(mirror, held)).collect();
    format!("[{}]", links.join(" > "))
}

/// Renders one link of a clip chain, with the space it is measured in unless that is the
/// identity.
fn link(mirror: &Mirror, link: &ClipLinkEntry) -> String {
    let mut text = match &link.shape {
        FrameClipShape::RoundedRect {
            rect: bounds,
            radii,
            shapes,
        } => {
            let mut text = rect(*bounds);
            if !all_zero(radii) {
                text.push_str(&format!(" radii={}", list(radii)));
            }
            if *shapes != ROUND {
                text.push_str(&format!(" shapes={}", list(shapes)));
            }
            text
        }
        FrameClipShape::Mask {
            rect: bounds,
            tile: mask,
            mode,
        } => format!(
            "mask {} tile={} mode={}",
            rect(*bounds),
            tile(*mask),
            if *mode == MaskMode::LUMINANCE {
                "luminance"
            } else {
                "alpha"
            }
        ),
    };

    if let Some(matrix) = matrix(mirror, link.space) {
        text.push_str(&format!(" in={matrix}"));
    }
    text
}

/// Renders the world matrix of a space slot, or returns `None` for the identity.
pub(crate) fn matrix(mirror: &Mirror, space: u32) -> Option<String> {
    let Some(matrix) = mirror.spaces.get(space as usize) else {
        return Some(format!("#{space} <missing>"));
    };
    if *matrix == Matrix4::IDENTITY {
        return None;
    }

    if let Some((x, y)) = matrix.plane_translation() {
        return Some(format!("translate{}", list(&[x, y])));
    }
    let columns: Vec<String> = matrix.columns.iter().map(|column| list(column)).collect();
    Some(format!("[{}]", columns.join(", ")))
}
