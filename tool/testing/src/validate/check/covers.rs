//! Opaque covers.

use iceblink::{BatchKind, FramePaint, PaintKind, PaintRef};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::rects::{output, show, within};
use crate::validate::check::resolve::Lanes;

/// The most covers a frame carries.
const MAX_COVERS: usize = 4;

/// Checks that every cover refers to an opaque quad of a quads batch drawn outside every group.
///
/// `depths` holds how many groups are open at each batch.
pub(crate) fn check(inspect: &mut Inspect<'_>, lanes: &Lanes<'_>, depths: &[usize]) {
    let frame = inspect.frame;
    if frame.opaque_covers.len() > MAX_COVERS {
        inspect.push(
            Rule::OpaqueCovers,
            format!("the frame has {} covers", frame.opaque_covers.len()),
        );
    }

    for (index, cover) in frame.opaque_covers.iter().enumerate() {
        if !within(cover.rect, output(frame)) {
            inspect.push(
                Rule::OpaqueCovers,
                format!("cover {index} rect {} leaves the output", show(cover.rect)),
            );
        }

        let Some(batch) = frame.batches.get(cover.batch as usize) else {
            inspect.push(
                Rule::OpaqueCovers,
                format!(
                    "cover {index} names batch {} of {}",
                    cover.batch,
                    frame.batches.len()
                ),
            );
            continue;
        };
        if batch.kind != BatchKind::Quads || !batch.range.contains(&cover.at) {
            inspect.push(
                Rule::OpaqueCovers,
                format!(
                    "cover {index} names quad {} of batch {} drawing {:?} {:?}",
                    cover.at, cover.batch, batch.kind, batch.range
                ),
            );
            continue;
        }
        if depths
            .get(cover.batch as usize)
            .is_some_and(|depth| *depth > 0)
        {
            inspect.push(
                Rule::OpaqueCovers,
                format!("cover {index} names a quad inside a group"),
            );
        }

        let Some(Some(quad)) = lanes.quads.get(cover.at as usize) else {
            continue;
        };
        let quad = quad.instance;
        let bordered = quad.border.iter().any(|width| *width > 0.0);
        if !opaque(inspect, quad.fill) || (bordered && !opaque(inspect, quad.stroke)) {
            inspect.push(
                Rule::OpaqueCovers,
                format!("cover {index} names quad {}, which is not opaque", cover.at),
            );
        }
    }
}

/// Reports whether `paint` is an opaque solid colour, or a slot the frames seen do not show.
fn opaque(inspect: &Inspect<'_>, paint: PaintRef) -> bool {
    if paint.kind != PaintKind::SOLID {
        return false;
    }

    match inspect.known.paints.get(paint.index).found() {
        Some(FramePaint::Solid(color)) => color.is_opaque(),
        Some(_) => false,
        None => true,
    }
}
