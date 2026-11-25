//! Atlas page and shader operations.

use iceblink::geom::{Point, Rect, Size};
use iceblink::{Frame, PageFormat, PageOp, RenderCapabilities, ShaderOp};

use crate::validate::Rule;
use crate::validate::check::Report;
use crate::validate::check::rects::{show, within};
use crate::validate::known::{Known, Lookup, Page};

/// Applies the frame's page and shader operations in order, checking each against what the
/// renderer holds.
pub(crate) fn apply(
    frame: &Frame,
    known: &mut Known,
    caps: Option<&RenderCapabilities>,
    report: &mut Report,
) {
    for (index, op) in frame.resources.pages.iter().enumerate() {
        page_op(index, op, known, caps, report);
    }

    for (index, op) in frame.resources.shaders.iter().enumerate() {
        shader_op(index, op, known, report);
    }
}

fn page_op(
    index: usize,
    op: &PageOp,
    known: &mut Known,
    caps: Option<&RenderCapabilities>,
    report: &mut Report,
) {
    match op {
        PageOp::Create { .. } => create(index, op, known, caps, report),
        PageOp::Upload { .. } => upload(index, op, known, report),
        PageOp::Destroy { page } => {
            if known.pages.remove(*page).is_missing() {
                report.push(
                    Rule::PageOps,
                    format!("page op {index} destroys page {page}, which does not exist"),
                );
            }
        }
    }
}

fn create(
    index: usize,
    op: &PageOp,
    known: &mut Known,
    caps: Option<&RenderCapabilities>,
    report: &mut Report,
) {
    let PageOp::Create {
        page,
        format,
        size,
        levels,
    } = op
    else {
        return;
    };

    if !format.is_valid() {
        report.push(
            Rule::Values,
            format!("page op {index} creates page {page} in format {}", format.0),
        );
    }
    let most = 32 - size[0].max(size[1]).max(1).leading_zeros();
    if *levels == 0 || *levels > most {
        report.push(
            Rule::Values,
            format!(
                "page op {index} creates page {page} of {}x{} with {levels} levels, \
                 outside 1 to {most}",
                size[0], size[1]
            ),
        );
    }
    if let Some(caps) = caps
        && size.iter().any(|side| *side > caps.max_texture_size)
    {
        report.push(
            Rule::Capabilities,
            format!(
                "page op {index} creates page {page} of {}x{}, past the limit of {}",
                size[0], size[1], caps.max_texture_size
            ),
        );
    }

    let page_state = Page {
        format: *format,
        size: *size,
        levels: (*levels).max(1),
    };
    if let Lookup::Found(()) = known.pages.insert(*page, page_state) {
        report.push(
            Rule::PageOps,
            format!("page op {index} creates page {page}, which exists"),
        );
    }
}

fn upload(index: usize, op: &PageOp, known: &Known, report: &mut Report) {
    let PageOp::Upload {
        page,
        level,
        rect,
        bytes,
        offset,
        stride,
    } = op
    else {
        return;
    };
    let (page, level, rect, bytes, offset, stride) =
        (*page, *level, *rect, bytes.len(), *offset, *stride);

    let held = match known.pages.get(page) {
        Lookup::Found(held) => Some(*held),
        Lookup::Missing => {
            report.push(
                Rule::PageOps,
                format!("page op {index} uploads to page {page}, which does not exist"),
            );
            return;
        }
        Lookup::Unknown => None,
    };

    if rect.is_empty() {
        return;
    }

    let Some(held) = held else {
        return;
    };
    if level >= held.levels {
        report.push(
            Rule::PageOps,
            format!(
                "page op {index} uploads to level {level} of page {page}, which has {}",
                held.levels
            ),
        );
        return;
    }
    let size = PageOp::level_size(held.size, level);
    let whole = Rect::new(Point::new(0, 0), Size::new(size[0] as i32, size[1] as i32));
    if rect.origin.x < 0 || rect.origin.y < 0 || !within(rect, whole) {
        report.push(
            Rule::PageOps,
            format!(
                "page op {index} uploads {} outside level {level} of page {page} of {}x{}",
                show(rect),
                size[0],
                size[1]
            ),
        );
    }

    let row = rect.size.width as usize * texel_bytes(held.format);
    let rows = rect.size.height as usize;
    let needed = offset + stride as usize * (rows - 1) + row;
    if (stride as usize) < row || needed > bytes {
        report.push(
            Rule::PageOps,
            format!(
                "page op {index} reads {needed} bytes at stride {stride} for {} from {bytes} bytes",
                show(rect)
            ),
        );
    }
}

/// Returns how many bytes one texel of `format` takes.
fn texel_bytes(format: PageFormat) -> usize {
    if format == PageFormat::A8 { 1 } else { 4 }
}

fn shader_op(index: usize, op: &ShaderOp, known: &mut Known, report: &mut Report) {
    match op {
        ShaderOp::Create { shader, .. } => {
            if let Lookup::Found(()) = known.shaders.insert(*shader, ()) {
                report.push(
                    Rule::ShaderOps,
                    format!("shader op {index} creates shader slot {shader}, which is live"),
                );
            }
        }
        ShaderOp::Destroy { shader } => {
            if known.shaders.remove(*shader).is_missing() {
                report.push(
                    Rule::ShaderOps,
                    format!("shader op {index} destroys shader slot {shader}, which is not live"),
                );
            }
        }
    }
}
