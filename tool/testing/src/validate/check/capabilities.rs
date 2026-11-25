//! The rules a renderer's capabilities add.

use iceblink::{Frame, PassKey, RenderCapabilities};

use crate::validate::Rule;
use crate::validate::check::Report;

/// Checks that the frame uses no resident chunk, shift, subpixel sprite or pass key that `caps`
/// rule out.
pub(crate) fn check(frame: &Frame, caps: &RenderCapabilities, report: &mut Report) {
    if !caps.resident_chunks {
        let lanes = &frame.lanes;
        let resident = [
            lanes.quads.sources.as_slice(),
            &lanes.shader_quads.sources,
            &lanes.shadows.sources,
            &lanes.lines.sources,
            &lanes.mono.sources,
            &lanes.subpixel.sources,
            &lanes.color.sources,
            &lanes.marks.sources,
        ]
        .iter()
        .any(|sources| sources.iter().any(|source| !source.is_transient()));
        if resident || !frame.chunk_notes.is_empty() || !frame.drawn.is_empty() {
            report.push(
                Rule::Capabilities,
                "the frame uses resident chunks, which the renderer does not keep",
            );
        }
    }

    if !caps.can_shift && !frame.shifts.is_empty() {
        report.push(
            Rule::Capabilities,
            "the frame shifts, which the renderer cannot",
        );
    }

    if !caps.subpixel_text && !frame.lanes.subpixel.sources.is_empty() {
        report.push(
            Rule::Capabilities,
            "the frame draws subpixel sprites, which the renderer cannot",
        );
    }

    let keyed = frame
        .vector_plan
        .passes
        .iter()
        .any(|pass| pass.key != PassKey::NONE);
    if !caps.kept_rasters && keyed {
        report.push(
            Rule::Capabilities,
            "the frame keys vector passes, which the renderer does not keep",
        );
    }
}
