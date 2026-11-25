//! The vector plan, the mark plan and the opaque covers.

use iceblink::{FRAME_NO_CLIP, Frame, PassKey};

use crate::capture::mirror::Mirror;
use crate::capture::text::Writer;
use crate::capture::transcript::instance::{self, device};
use crate::capture::transcript::space;

/// Writes the composite of vector pass `index` with the paths it rasterises nested beneath it.
pub(crate) fn pass(writer: &mut Writer, frame: &Frame, mirror: &Mirror, index: u32) {
    let plan = &frame.vector_plan;
    let Some(pass) = plan.passes.get(index as usize) else {
        writer.line(&format!("paths pass={index} <missing>"));
        return;
    };

    let mut header = format!("paths pass={index} region={}", device(pass.region));
    if pass.clip != FRAME_NO_CLIP {
        header.push_str(&format!(" clip={}", space::chain(mirror, pass.clip)));
    }
    for (set, name) in [
        (pass.instanced, "instanced"),
        (pass.clamped, "clamped"),
        (pass.overhangs, "overhangs"),
        (pass.key != PassKey::NONE, "keyed"),
    ] {
        if set {
            header.push(' ');
            header.push_str(name);
        }
    }

    let start = pass.items.start as usize;
    let end = (pass.items.end as usize).min(plan.items.len());
    writer.nested(&header, |writer| {
        for planned in plan.items.get(start..end).unwrap_or_default() {
            let Some(path) = frame.paths.get(planned.path as usize) else {
                writer.line(&format!("path {} <missing>", planned.path));
                continue;
            };
            let mut line = instance::path(mirror, path);
            if planned.residual != FRAME_NO_CLIP {
                line.push_str(&format!(
                    " residual={}",
                    space::chain(mirror, planned.residual)
                ));
            }
            if pass.instanced && planned.clip != FRAME_NO_CLIP {
                line.push_str(&format!(" clip={}", space::chain(mirror, planned.clip)));
            }
            writer.line(&line);
        }
    });
}

/// Renders the bin of the marks instance at `at` of the marks lane, or nothing when it has none.
pub(crate) fn bin(frame: &Frame, at: u32) -> String {
    let plan = &frame.mark_plan;
    if let Some(bin) = plan.bin(at) {
        return format!(
            " bin=page{} at=[{}, {}] region={}",
            bin.page,
            bin.at[0],
            bin.at[1],
            device(bin.region)
        );
    }
    if let Some(bin) = plan.overflow_bin(at) {
        return format!(" overflow_bin region={}", device(bin.region));
    }
    String::new()
}

/// Writes the mark plan's pages, when it has bins.
pub(crate) fn mark_pages(writer: &mut Writer, frame: &Frame) {
    let plan = &frame.mark_plan;
    if plan.is_empty() {
        return;
    }
    writer.line(&format!(
        "mark_pages {} extent={}x{}{}",
        plan.pages,
        plan.extent[0],
        plan.extent[1],
        if plan.full_damage { " full_damage" } else { "" }
    ));
}

/// Writes the opaque covers, when there are any.
pub(crate) fn covers(writer: &mut Writer, frame: &Frame) {
    for cover in &frame.opaque_covers {
        writer.line(&format!(
            "cover {} batch={} at={}",
            device(cover.rect),
            cover.batch,
            cover.at
        ));
    }
}
