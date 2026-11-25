//! The frame's resource work, chunk notes and table deltas.

use iceblink::{
    ChunkNote, Frame, LoweredChunk, PageFormat, PageOp, ShaderMode, ShaderOp, TableDelta,
};

use crate::capture::text::Writer;
use crate::capture::transcript::instance::device;

/// Writes the work the renderer applies before drawing, when there is any.
pub(crate) fn work(writer: &mut Writer, frame: &Frame) {
    let mut lines: Vec<String> = frame.resources.pages.iter().map(page).collect();
    lines.extend(frame.resources.shaders.iter().map(shader));
    lines.extend(frame.chunk_notes.iter().map(note));
    let tables = &frame.tables;
    for (name, delta) in [
        ("spaces", delta(&tables.spaces)),
        ("clips", delta(&tables.clips)),
        ("paints", delta(&tables.paints)),
        ("params", delta(&tables.params)),
    ] {
        if let Some(delta) = delta {
            lines.push(format!("table {name} {delta}"));
        }
    }

    if lines.is_empty() {
        return;
    }
    writer.nested(&format!("work {}", lines.len()), |writer| {
        for line in &lines {
            writer.line(line);
        }
    });
}

fn page(op: &PageOp) -> String {
    match op {
        PageOp::Create {
            page,
            format,
            size,
            levels,
        } => {
            let mut line = format!(
                "page create {page} {} {}x{}",
                page_format(*format),
                size[0],
                size[1]
            );
            if *levels > 1 {
                line.push_str(&format!(" levels={levels}"));
            }
            line
        }
        PageOp::Upload {
            page,
            level,
            rect,
            bytes,
            ..
        } => {
            let mut line = format!("page upload {page}");
            if *level > 0 {
                line.push_str(&format!(" level={level}"));
            }
            line.push_str(&format!(" {} bytes={}", device(*rect), bytes.len()));
            line
        }
        PageOp::Destroy { page } => format!("page destroy {page}"),
    }
}

fn shader(op: &ShaderOp) -> String {
    match op {
        ShaderOp::Create { shader, desc } => {
            format!("shader create #{shader} mode={}", mode(desc.mode))
        }
        ShaderOp::Destroy { shader } => format!("shader destroy #{shader}"),
    }
}

fn note(note: &ChunkNote) -> String {
    match note {
        ChunkNote::Inserted { revision, chunk } => {
            format!("note inserted {revision}{}", counts(chunk))
        }
        ChunkNote::Retired { revision } => format!("note retired {revision}"),
        ChunkNote::ResetAll => "note reset_all".to_owned(),
    }
}

fn page_format(format: PageFormat) -> String {
    match format {
        PageFormat::A8 => "a8".to_owned(),
        PageFormat::RGBA8_PREMULTIPLIED => "rgba8_premultiplied".to_owned(),
        PageFormat::SUBPIXEL => "subpixel".to_owned(),
        other => format!("<format {}>", other.0),
    }
}

/// Renders how many instances of each kind a lowered chunk holds, leaving out empty kinds.
fn counts(chunk: &LoweredChunk) -> String {
    let mut text = String::new();
    for (name, count) in [
        ("quads", chunk.quads.len()),
        ("shader_quads", chunk.shader_quads.len()),
        ("shadows", chunk.shadows.len()),
        ("lines", chunk.lines.len()),
        ("mono", chunk.mono.len()),
        ("subpixel", chunk.subpixel.len()),
        ("color", chunk.color.len()),
        ("marks", chunk.marks.len()),
    ] {
        if count > 0 {
            text.push_str(&format!(" {name}={count}"));
        }
    }
    text
}

/// Renders the slots a delta writes, or returns `None` when it leaves the table unchanged.
fn delta<T>(delta: &TableDelta<T>) -> Option<String> {
    if delta.is_unchanged() {
        return None;
    }

    let mut text = String::new();
    if delta.replace {
        text.push_str(&format!("replace={}", delta.all.len()));
    }
    if !delta.changed.is_empty() {
        let slots: Vec<String> = delta
            .changed
            .iter()
            .map(|(slot, _)| slot.to_string())
            .collect();
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&format!("changed=[{}]", slots.join(", ")));
    }
    Some(text)
}

fn mode(mode: ShaderMode) -> String {
    match mode {
        ShaderMode::PAINT => "paint".to_owned(),
        ShaderMode::COVERAGE => "coverage".to_owned(),
        ShaderMode::FILTER => "filter".to_owned(),
        other => format!("<mode {}>", other.0),
    }
}
