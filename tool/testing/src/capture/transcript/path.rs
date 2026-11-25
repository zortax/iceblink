//! Path geometry, stroke styles and fill rules under the transcript's number rules.

use iceblink::FillRule;
use iceblink::kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Stroke};

use crate::capture::text::number::{float, list};

/// Renders a path as SVG-style commands.
pub(crate) fn commands(path: &BezPath) -> String {
    let mut text = String::new();
    for element in path.elements() {
        if !text.is_empty() {
            text.push(' ');
        }
        match element {
            PathEl::MoveTo(to) => text.push_str(&format!("M {}", point(*to))),
            PathEl::LineTo(to) => text.push_str(&format!("L {}", point(*to))),
            PathEl::QuadTo(control, to) => {
                text.push_str(&format!("Q {} {}", point(*control), point(*to)));
            }
            PathEl::CurveTo(first, second, to) => text.push_str(&format!(
                "C {} {} {}",
                point(*first),
                point(*second),
                point(*to)
            )),
            PathEl::ClosePath => text.push('Z'),
        }
    }
    text
}

/// Renders an affine map, or returns `None` for the identity.
pub(crate) fn affine(map: Affine) -> Option<String> {
    if map == Affine::IDENTITY {
        return None;
    }
    let coefficients = map.as_coeffs().map(|value| value as f32);
    Some(list(&coefficients))
}

fn point(point: Point) -> String {
    format!("{},{}", float(point.x as f32), float(point.y as f32))
}

/// Renders the width of a stroke, and every setting that differs from a default stroke of that
/// width.
pub(crate) fn stroke(style: &Stroke) -> String {
    let mut text = format!(" width={}", float(style.width as f32));
    let plain = Stroke::new(style.width);
    if style.start_cap != plain.start_cap || style.end_cap != plain.end_cap {
        text.push_str(&format!(
            " caps={}/{}",
            cap(style.start_cap),
            cap(style.end_cap)
        ));
    }
    if style.join != plain.join {
        text.push_str(&format!(" join={}", join(style.join)));
    }
    if style.miter_limit != plain.miter_limit {
        text.push_str(&format!(" miter={}", float(style.miter_limit as f32)));
    }
    if !style.dash_pattern.is_empty() {
        let dashes: Vec<f32> = style.dash_pattern.iter().map(|dash| *dash as f32).collect();
        text.push_str(&format!(
            " dashes={}@{}",
            list(&dashes),
            float(style.dash_offset as f32)
        ));
    }
    text
}

fn cap(cap: Cap) -> &'static str {
    match cap {
        Cap::Butt => "butt",
        Cap::Square => "square",
        Cap::Round => "round",
    }
}

fn join(join: Join) -> &'static str {
    match join {
        Join::Bevel => "bevel",
        Join::Miter => "miter",
        Join::Round => "round",
    }
}

pub(crate) fn rule(rule: FillRule) -> String {
    match rule {
        FillRule::NON_ZERO => "nonzero".to_owned(),
        FillRule::EVEN_ODD => "evenodd".to_owned(),
        other => format!("<rule {}>", other.0),
    }
}
