//! Copying a frame, which is not `Clone`.

use iceblink::{Frame, Lane, Lanes};

/// Returns a copy of `frame` that shares its `Arc`s.
pub fn copy_frame(frame: &Frame) -> Frame {
    Frame {
        id: frame.id,
        output: frame.output,
        size: frame.size,
        scale: frame.scale,
        opaque: frame.opaque,
        clock: frame.clock,
        shader_clock: frame.shader_clock,
        damage: frame.damage,
        shifts: frame.shifts.clone(),
        chunk_notes: frame.chunk_notes.clone(),
        drawn: frame.drawn.clone(),
        lanes: copy_lanes(&frame.lanes),
        paths: frame.paths.clone(),
        externals: frame.externals.clone(),
        backdrops: frame.backdrops.clone(),
        groups: frame.groups.clone(),
        batches: frame.batches.clone(),
        vector_plan: frame.vector_plan.clone(),
        mark_plan: frame.mark_plan.clone(),
        opaque_covers: frame.opaque_covers.clone(),
        tables: frame.tables.clone(),
        resources: frame.resources.clone(),
    }
}

fn copy_lanes(lanes: &Lanes) -> Lanes {
    Lanes {
        quads: copy_lane(&lanes.quads),
        shader_quads: copy_lane(&lanes.shader_quads),
        shadows: copy_lane(&lanes.shadows),
        lines: copy_lane(&lanes.lines),
        mono: copy_lane(&lanes.mono),
        subpixel: copy_lane(&lanes.subpixel),
        color: copy_lane(&lanes.color),
        marks: copy_lane(&lanes.marks),
        mark_payloads: lanes.mark_payloads.clone(),
    }
}

fn copy_lane<T: Copy>(lane: &Lane<T>) -> Lane<T> {
    Lane {
        items: lane.items.clone(),
        sources: lane.sources.clone(),
    }
}
