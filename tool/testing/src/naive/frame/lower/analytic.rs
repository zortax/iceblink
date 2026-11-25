//! Quads, shader quads, shadows and lines, which keep their client layouts in the frame.

use iceblink::{
    BatchKind, BorderStyle, ClipRef, FrameShadow, Instance, Line, Quad, ShaderMode, ShaderQuad,
    Shadow, SpaceRef,
};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::{self, Placed};
use crate::naive::geometry::Bounds;
use crate::naive::scene::Chunk;

pub(crate) fn quad(builder: &mut Builder<'_>, chunk: &Chunk, quad: &Quad) -> Bounds {
    let Some(placed) = item::place(builder, chunk, quad.space, quad.clip) else {
        return Bounds::EMPTY;
    };

    let ink = placed.ink(Bounds::of(quad.bounds), builder.env.output);
    if item::shows(builder, ink) {
        push_quad(builder, &placed, *quad);
    }
    ink
}

fn push_quad(builder: &mut Builder<'_>, placed: &Placed, quad: Quad) {
    let mut lowered = quad.moved(placed.offset);
    lowered.fill = builder.paint(quad.fill);
    lowered.stroke = builder.paint(quad.stroke);
    lowered.clip = ClipRef(placed.clip);
    lowered.space = SpaceRef(placed.space);
    builder.instance(|lanes| &mut lanes.quads, lowered, BatchKind::Quads);
}

/// Lowers a shader quad.
///
/// When the renderer runs no application shaders, a coverage-mode quad is drawn as the quad with
/// the same fields and a solid border, and a paint-mode quad draws nothing.
pub(crate) fn shader_quad(builder: &mut Builder<'_>, chunk: &Chunk, quad: &ShaderQuad) -> Bounds {
    let mode = builder
        .env
        .client
        .shaders
        .at(quad.shader)
        .map(|desc| desc.mode);
    if !builder.env.caps.custom_shaders && mode != Some(ShaderMode::COVERAGE) {
        return Bounds::EMPTY;
    }

    let Some(placed) = item::place(builder, chunk, quad.space, quad.clip) else {
        return Bounds::EMPTY;
    };

    let ink = placed.ink(Bounds::of(quad.bounds), builder.env.output);
    if !item::shows(builder, ink) {
        return ink;
    }

    if !builder.env.caps.custom_shaders {
        let plain = Quad {
            bounds: quad.bounds,
            radii: quad.radii,
            shapes: quad.shapes,
            border: quad.border,
            fill: quad.fill,
            stroke: quad.stroke,
            paint_origin: quad.paint_origin,
            clip: quad.clip,
            space: quad.space,
            style: BorderStyle::SOLID,
            _pad: [0; 3],
        };
        push_quad(builder, &placed, plain);
        return ink;
    }

    let Some(shader) = builder.shader(quad.shader) else {
        return ink;
    };
    let params = builder.params(quad.params);
    let mut lowered = quad.moved(placed.offset);
    lowered.fill = builder.paint(quad.fill);
    lowered.stroke = builder.paint(quad.stroke);
    lowered.clip = ClipRef(placed.clip);
    lowered.space = SpaceRef(placed.space);
    lowered.shader = shader;
    lowered.params = params;
    builder.instance(
        |lanes| &mut lanes.shader_quads,
        lowered,
        BatchKind::ShaderQuads { shader, params },
    );
    ink
}

pub(crate) fn shadow(builder: &mut Builder<'_>, chunk: &Chunk, shadow: &Shadow) -> Bounds {
    let Some(placed) = item::place(builder, chunk, shadow.space, shadow.clip) else {
        return Bounds::EMPTY;
    };

    let local = shadow_ink(shadow);
    let ink = placed.ink(local, builder.env.output);
    if !item::shows(builder, ink) {
        return ink;
    }

    let mut lowered = *shadow;
    lowered.color = shadow.color.quantized();
    lowered.clip = ClipRef(placed.clip);
    lowered.space = SpaceRef(placed.space);
    let instance = FrameShadow {
        bounds: local.to_rect(),
        shadow: lowered,
    }
    .moved(placed.offset);
    builder.instance(|lanes| &mut lanes.shadows, instance, BatchKind::Shadows);
    ink
}

/// Returns a shadow's local ink: the shape outset by three deviations for a drop shadow, and the
/// caster for an inset one.
fn shadow_ink(shadow: &Shadow) -> Bounds {
    if shadow.inset == 1 {
        Bounds::of(shadow.caster_bounds)
    } else {
        Bounds::of(shadow.shape_bounds).outset(3.0 * shadow.blur.max(0.0))
    }
}

pub(crate) fn line(builder: &mut Builder<'_>, chunk: &Chunk, line: &Line) -> Bounds {
    let Some(placed) = item::place(builder, chunk, line.space, line.clip) else {
        return Bounds::EMPTY;
    };

    let ink = placed.ink(Bounds::of(line.bounds), builder.env.output);
    if item::shows(builder, ink) {
        let mut lowered = line.moved(placed.offset);
        lowered.color = line.color.quantized();
        lowered.clip = ClipRef(placed.clip);
        lowered.space = SpaceRef(placed.space);
        builder.instance(|lanes| &mut lanes.lines, lowered, BatchKind::Lines);
    }
    ink
}
