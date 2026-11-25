//! The keys of vector passes.

use std::hash::{DefaultHasher, Hasher};
use std::sync::Arc;

use iceblink::kurbo;
use iceblink::{FRAME_NO_CLIP, Frame, FramePaint, PaintKind, PaintRef, PassKey, PlannedPass};

use crate::validate::Rule;
use crate::validate::check::Report;
use crate::validate::known::{Known, Table};

/// Checks that every keyed pass is unclamped and has no residual clip, and that no key is issued
/// for content other than what it was first issued for.
///
/// The content is the region's size and, for each path, its outline and clip outlines by their
/// elements, its matrix relative to the region's origin, its brush, fill, stroke and paints, and
/// its ink relative to the region. A pass with a paint the frames seen so far do not hold is not
/// compared.
pub(crate) fn check(frame: &Frame, known: &mut Known, report: &mut Report) {
    for (index, pass) in frame.vector_plan.passes.iter().enumerate() {
        if pass.key == PassKey::NONE {
            continue;
        }

        let items = frame.vector_plan.items_of(pass);
        if pass.clamped {
            report.push(Rule::VectorPlan, format!("keyed pass {index} is clamped"));
        }
        if items.iter().any(|item| item.residual != FRAME_NO_CLIP) {
            report.push(
                Rule::VectorPlan,
                format!("keyed pass {index} has a path with a residual clip"),
            );
        }

        let Some(digest) = digest(frame, &known.paints, pass) else {
            continue;
        };
        let outlines = || {
            items
                .iter()
                .filter_map(|item| frame.paths.get(item.path as usize))
                .flat_map(|path| {
                    core::iter::once(Arc::clone(&path.path))
                        .chain(path.clips.iter().map(|clip| Arc::clone(&clip.path)))
                })
                .collect()
        };
        if known.keys.issue(pass.key, digest, outlines).is_some() {
            report.push(
                Rule::VectorPlan,
                format!(
                    "pass {index} has key {:#x}, which was issued for other content",
                    pass.key.0
                ),
            );
        }
    }
}

/// Returns the digest of what `pass` rasterises, or `None` when a paint it reads is not known.
fn digest(frame: &Frame, paints: &Table<FramePaint>, pass: &PlannedPass) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    hasher.write_i32(pass.region.size.width);
    hasher.write_i32(pass.region.size.height);
    let origin = [f64::from(pass.region.left()), f64::from(pass.region.top())];
    for item in frame.vector_plan.items_of(pass) {
        let path = frame.paths.get(item.path as usize)?;
        outline(&mut hasher, &path.path);
        for clip in path.clips.iter() {
            outline(&mut hasher, &clip.path);
            hasher.write_u8(clip.rule.0);
        }
        let mut transform = path.transform.as_coeffs();
        transform[4] -= origin[0];
        transform[5] -= origin[1];
        for value in transform.iter().chain(&path.brush.as_coeffs()) {
            hasher.write_u64(value.to_bits());
        }
        if let Some(fill) = &path.fill {
            hasher.write_u8(fill.rule.0);
            paint(paints, &mut hasher, fill.paint)?;
        }
        if let Some(stroke) = &path.stroke {
            hasher.write(format!("{:?}", stroke.style).as_bytes());
            paint(paints, &mut hasher, stroke.paint)?;
        }
        for value in [
            item.ink.left(),
            item.ink.top(),
            item.ink.size.width,
            item.ink.size.height,
        ] {
            hasher.write_i32(value);
        }
    }
    Some(hasher.finish())
}

fn outline(hasher: &mut DefaultHasher, path: &kurbo::BezPath) {
    for element in path.elements() {
        hasher.write(format!("{element:?}").as_bytes());
    }
}

/// Hashes the paint `reference` refers to, or returns `None` when the frames seen so far do not
/// hold it.
fn paint(
    paints: &Table<FramePaint>,
    hasher: &mut DefaultHasher,
    reference: PaintRef,
) -> Option<()> {
    if reference.kind == PaintKind::NONE {
        hasher.write_u8(0);
        return Some(());
    }

    let paint = paints.get(reference.index).found()?;
    hasher.write(format!("{paint:?}").as_bytes());
    Some(())
}
