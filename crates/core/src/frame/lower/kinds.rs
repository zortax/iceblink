//! Lowering quads, shader quads, shadows and lines.

use iceblink::{BorderStyle, FrameShadow, Line, Quad, ShaderMode, ShaderQuad, Shadow};

use crate::frame::lower::Slots;
use crate::frame::lower::build::{Build, push};
use crate::geometry::ink::shadow_ink;
use crate::scene::chunk::{Lane, LoweredItem, LoweringKey};
use crate::tables::FrameTables;

pub(super) fn quad(
    slots: &Slots<'_>,
    tables: &mut FrameTables,
    build: &mut Build,
    mut quad: Quad,
) -> LoweredItem {
    quad.fill = slots.paint(tables, quad.fill);
    quad.stroke = slots.paint(tables, quad.stroke);
    quad.clip = slots.clip(tables, quad.clip);
    quad.space = slots.space(quad.space);
    push(&mut build.chunk.quads, Lane::Quads, quad, build.base)
}

/// Lowers a shader quad, or the quad that stands for it when the renderer has no custom shaders.
pub(super) fn shader_quad(
    slots: &Slots<'_>,
    tables: &mut FrameTables,
    key: LoweringKey,
    build: &mut Build,
    item: ShaderQuad,
) -> LoweredItem {
    let mode = slots
        .scene
        .shaders
        .at(item.shader)
        .map(|shader| shader.desc.mode);
    if !key.custom_shaders {
        if mode != Some(ShaderMode::COVERAGE) {
            return LoweredItem::Nothing;
        }
        let quad = Quad {
            bounds: item.bounds,
            radii: item.radii,
            shapes: item.shapes,
            border: item.border,
            fill: slots.paint(tables, item.fill),
            stroke: slots.paint(tables, item.stroke),
            paint_origin: item.paint_origin,
            clip: slots.clip(tables, item.clip),
            space: slots.space(item.space),
            style: BorderStyle::SOLID,
            _pad: [0; 3],
        };
        return push(&mut build.chunk.quads, Lane::Quads, quad, build.base);
    }

    let mut quad = item;
    quad.fill = slots.paint(tables, quad.fill);
    quad.stroke = slots.paint(tables, quad.stroke);
    quad.clip = slots.clip(tables, quad.clip);
    quad.space = slots.space(quad.space);
    quad.shader = slots.shader(quad.shader);
    quad.params = slots.params(quad.params);
    push(
        &mut build.chunk.shader_quads,
        Lane::ShaderQuads,
        quad,
        build.base,
    )
}

pub(super) fn shadow(
    slots: &Slots<'_>,
    tables: &mut FrameTables,
    build: &mut Build,
    item: Shadow,
) -> LoweredItem {
    let mut shadow = item;
    shadow.color = shadow.color.quantized();
    shadow.clip = slots.clip(tables, shadow.clip);
    shadow.space = slots.space(shadow.space);
    let frame = FrameShadow {
        bounds: shadow_ink(&item),
        shadow,
    };
    push(&mut build.chunk.shadows, Lane::Shadows, frame, build.base)
}

pub(super) fn line(
    slots: &Slots<'_>,
    tables: &mut FrameTables,
    build: &mut Build,
    mut line: Line,
) -> LoweredItem {
    line.color = line.color.quantized();
    line.clip = slots.clip(tables, line.clip);
    line.space = slots.space(line.space);
    push(&mut build.chunk.lines, Lane::Lines, line, build.base)
}
