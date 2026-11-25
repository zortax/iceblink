//! The raw slots that the items of a chunk refer to.

use iceblink::{ClipRef, Filter, PaintKind, PaintRef, RecordedChunk, SpaceRef};

/// A slot an item refers to, by the kind of object in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Slot {
    Transform(u32),
    Clip(u32),
    Paint(u32),
    Params(u32),
    Shader(u32),
    Image(u32),
    Texture(u32),
}

/// Calls `visit` with every slot an item or a filter chain of `chunk` refers to.
///
/// Inherited spaces and clips, local clips and inline paints refer to no slot.
pub(super) fn for_each(chunk: &RecordedChunk, visit: &mut dyn FnMut(Slot)) {
    for item in &chunk.quads {
        space(item.space, visit);
        clip(item.clip, visit);
        paint(item.fill, visit);
        paint(item.stroke, visit);
    }

    for item in &chunk.shader_quads {
        space(item.space, visit);
        clip(item.clip, visit);
        paint(item.fill, visit);
        paint(item.stroke, visit);
        visit(Slot::Shader(item.shader));
        visit(Slot::Params(item.params));
    }

    for item in &chunk.shadows {
        space(item.space, visit);
        clip(item.clip, visit);
    }

    for item in &chunk.lines {
        space(item.space, visit);
        clip(item.clip, visit);
    }

    for item in &chunk.glyph_runs {
        space(item.space, visit);
        clip(item.clip, visit);
        paint(item.paint, visit);
    }

    for item in &chunk.images {
        space(item.space, visit);
        clip(item.clip, visit);
        paint(item.paint, visit);
        visit(Slot::Image(item.image));
    }

    for item in &chunk.marks {
        space(item.space, visit);
        clip(item.clip, visit);
        paint(item.paint, visit);
    }

    for item in &chunk.paths {
        space(item.space, visit);
        clip(item.clip, visit);
        if let Some(fill) = &item.fill {
            paint(fill.paint, visit);
        }
        if let Some(stroke) = &item.stroke {
            paint(stroke.paint, visit);
        }
    }

    for item in &chunk.externals {
        space(item.space, visit);
        clip(item.clip, visit);
        visit(Slot::Texture(item.texture));
    }

    for item in &chunk.backdrops {
        space(item.space, visit);
        clip(item.clip, visit);
    }

    for chain in &chunk.filters {
        for filter in chain.iter() {
            if let Filter::Shader { shader, params, .. } = filter {
                visit(Slot::Shader(shader.slot()));
                visit(Slot::Params(params.slot()));
            }
        }
    }
}

fn space(space: SpaceRef, visit: &mut dyn FnMut(Slot)) {
    if space != SpaceRef::INHERIT {
        visit(Slot::Transform(space.0));
    }
}

fn clip(clip: ClipRef, visit: &mut dyn FnMut(Slot)) {
    if clip != ClipRef::INHERIT && clip != ClipRef::NONE && clip.local_index().is_none() {
        visit(Slot::Clip(clip.0));
    }
}

fn paint(paint: PaintRef, visit: &mut dyn FnMut(Slot)) {
    if paint.kind != PaintKind::NONE && paint.kind != PaintKind::INLINE {
        visit(Slot::Paint(paint.index));
    }
}
