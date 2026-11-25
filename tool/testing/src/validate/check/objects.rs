//! Paths, externals, backdrops and groups: the frame kinds drawn one at a time.

use iceblink::{BatchKind, Frame};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::rects::finite;

/// Checks the frame slots and values of every path, external, backdrop and group.
pub(crate) fn check(inspect: &mut Inspect<'_>) {
    paths(inspect);
    externals(inspect);
    backdrops(inspect);
    groups(inspect);
}

fn paths(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    for (index, path) in frame.paths.iter().enumerate() {
        let at = || format!("path {index}");
        if let Some(fill) = &path.fill {
            inspect.paint(fill.paint, at);
            if !fill.rule.is_valid() {
                inspect.push(
                    Rule::Values,
                    format!("{} has fill rule {}", at(), fill.rule.0),
                );
            }
        }
        if let Some(stroke) = &path.stroke {
            inspect.paint(stroke.paint, at);
        }
    }
}

fn externals(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    for (index, external) in frame.externals.iter().enumerate() {
        let at = || format!("external {index}");
        inspect.clip(external.quad.clip.0, at);
        inspect.space(external.quad.space.0, at);
    }
}

fn backdrops(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    for (index, backdrop) in frame.backdrops.iter().enumerate() {
        let at = || format!("backdrop {index}");
        inspect.clip(backdrop.backdrop.clip.0, at);
        inspect.space(backdrop.backdrop.space.0, at);
        inspect.filters(&backdrop.filters, at);
        if !finite(&backdrop.bounds) || !finite(&backdrop.source) {
            inspect.push(Rule::Values, format!("{} has a non-finite rectangle", at()));
        }
    }
}

fn groups(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let parents = parents(frame);
    for (index, group) in frame.groups.iter().enumerate() {
        let at = || format!("group {index}");
        inspect.clip(group.clip, at);
        inspect.filters(&group.filters, at);
        if !group.blend.is_valid() {
            inspect.push(
                Rule::Values,
                format!("{} has blend mode {}", at(), group.blend.0),
            );
        }
        if !finite(&group.bounds) || !finite(&group.source) || !group.opacity.is_finite() {
            inspect.push(
                Rule::Values,
                format!("{} has a non-finite bound or opacity", at()),
            );
            continue;
        }

        let within = parents
            .get(index)
            .copied()
            .flatten()
            .and_then(|parent| frame.groups.get(parent));
        let fits = inside(group.bounds, frame.size)
            || within.is_some_and(|parent| contains(parent.source, group.bounds));
        if !fits {
            inspect.push(
                Rule::GroupBounds,
                format!(
                    "{} has bounds {:?}, which leave the output or the source of its group",
                    at(),
                    group.bounds
                ),
            );
        }
        if !contains(group.source, group.bounds) {
            inspect.push(
                Rule::GroupBounds,
                format!(
                    "{} has source {:?}, which does not hold its bounds {:?}",
                    at(),
                    group.source,
                    group.bounds
                ),
            );
        }
    }
}

/// Returns the group each group's markers lie inside, by index in `Frame::groups`.
fn parents(frame: &Frame) -> Vec<Option<usize>> {
    let mut parents = vec![None; frame.groups.len()];
    let mut open: Vec<usize> = Vec::new();
    for batch in &frame.batches {
        match batch.kind {
            BatchKind::GroupBegin(index) => {
                if let Some(parent) = parents.get_mut(index as usize) {
                    *parent = open.last().copied();
                }
                open.push(index as usize);
            }
            BatchKind::GroupEnd(_) => {
                open.pop();
            }
            _ => {}
        }
    }
    parents
}

/// Reports whether device rectangle `outer` holds `inner`.
///
/// Every rectangle holds an empty one.
fn contains(outer: [f32; 4], [x, y, width, height]: [f32; 4]) -> bool {
    if width <= 0.0 || height <= 0.0 {
        return true;
    }
    let [left, top, outer_width, outer_height] = outer;
    x >= left && y >= top && x + width <= left + outer_width && y + height <= top + outer_height
}

/// Reports whether a device rectangle lies inside an output of the given size.
///
/// An empty rectangle lies inside every output.
fn inside([x, y, width, height]: [f32; 4], [output_width, output_height]: [u32; 2]) -> bool {
    if width <= 0.0 || height <= 0.0 {
        return true;
    }
    x >= 0.0 && y >= 0.0 && x + width <= output_width as f32 && y + height <= output_height as f32
}
