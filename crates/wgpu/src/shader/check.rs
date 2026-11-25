//! The check that the modules' structures and loaders match the Rust layouts.

use crate::shader::reflect::{self, Reflected, reflected};
use crate::shader::{Module, loaders};

/// Returns the Rust structures a module declares a WGSL twin of.
fn structures(module: Module) -> Vec<Reflected> {
    use crate::bind::globals::Globals;
    use crate::bind::tables::{GpuClip, GpuClipLink, GpuPaint, GpuStop};
    use iceblink::{ColorSprite, FrameMarks, FrameShadow, Line, MarkBox, Quad, Sprite};

    if !module.uses_tables() {
        return match module {
            Module::Blur => vec![reflected!(
                crate::pipeline::blur::BlurParams,
                "BlurParams",
                [extents, kernel, sampling, valid, origins, edge]
            )],
            _ => Vec::new(),
        };
    }

    let mut structures = vec![
        reflected!(
            Globals,
            "cr_Globals",
            [viewport, gamma_ratios, text, frame, origin]
        ),
        reflected!(
            GpuClip,
            "cr_Clip",
            [
                aabb, first, second, kept_aabb, count, space, kept_tests, links
            ]
        ),
        reflected!(
            GpuClipLink,
            "cr_ClipLink",
            [kind, space, mode, count, rect, radii, extra]
        ),
        reflected!(
            GpuPaint,
            "cr_Paint",
            [
                kind, gradient, space, flags, geometry, color, stop_start, stop_count, pad0, pad1
            ]
        ),
        reflected!(GpuStop, "cr_Stop", [color, offset, pad]),
    ];
    if module.is_marks() {
        structures.push(reflected!(
            FrameMarks,
            "MarkItem",
            [
                marks.bounds,
                marks.paint,
                marks.paint_origin,
                marks.origin,
                marks.axes,
                marks.half_width,
                marks.discs,
                marks.boxes,
                marks.vertices,
                marks.placements,
                marks.first,
                marks.payload,
                marks.clip,
                marks.space,
                marks.flags,
                page
            ]
        ));
        structures.push(reflected!(
            crate::pipeline::marks::MarkDraw,
            "MarkDraw",
            [position, page, shift, region, cells]
        ));
        if module == Module::MarksBox {
            structures.push(reflected!(MarkBox, "MarkBox", [rect, radii, shape]));
        }
        return structures;
    }
    let own = match module {
        Module::Quad => reflected!(
            Quad,
            "Quad",
            [
                bounds,
                radii,
                shapes,
                border,
                fill,
                stroke,
                paint_origin,
                clip,
                space,
                style,
                _pad
            ]
        ),
        Module::Shadow => reflected!(
            FrameShadow,
            "Shadow",
            [
                bounds,
                shadow.shape_bounds,
                shadow.radii,
                shadow.caster_bounds,
                shadow.caster_radii,
                shadow.shapes,
                shadow.color,
                shadow.blur,
                shadow.clip,
                shadow.space,
                shadow.inset
            ]
        ),
        Module::Line => reflected!(
            Line,
            "Line",
            [bounds, color, thickness, clip, space, style, _pad]
        ),
        Module::MonoSprite | Module::SubpixelSprite => reflected!(
            Sprite,
            "Sprite",
            [bounds, paint, paint_origin, tile, clip, space, _pad]
        ),
        Module::ColorSprite => reflected!(
            ColorSprite,
            "ColorSprite",
            [
                bounds,
                frame,
                radii,
                shapes,
                tile,
                paint,
                paint_origin,
                opacity,
                clip,
                space,
                flags
            ]
        ),
        Module::Composite => reflected!(
            crate::pipeline::composite::CompositeParams,
            "CompositeParams",
            [
                bounds,
                source,
                control,
                tint,
                matrix0,
                matrix1,
                matrix2,
                matrix3,
                matrix_offset,
                origin,
                written
            ]
        ),
        Module::External => reflected!(
            crate::pipeline::external::ExternalParams,
            "ExternalParams",
            [bounds, control]
        ),
        Module::Vector => reflected!(
            crate::pipeline::vector::VectorInstance,
            "VectorInstance",
            [bounds, source, control]
        ),
        _ => unreachable!("every other module was answered above"),
    };
    structures.push(own);
    structures
}

/// Checks that the structures and loaders of the renderer's shader modules match the Rust layouts
/// the renderer writes.
///
/// The `cr_ShaderQuad` of the application shader epilogue is checked too.
///
/// Fails with a description of the first mismatch, of a structure a module does not declare, or of
/// a check that compared fewer than 42 loaders.
pub fn check_shader_layouts() -> Result<(), String> {
    let mut loaders = 0;
    for module in Module::ALL {
        let named = |error: String| format!("{}: {error}", module.label());
        reflect::check(module.source(), &structures(module)).map_err(named)?;
        loaders +=
            loaders::check_loaders(module.source(), &module.representation()).map_err(named)?;
    }
    // The 14 modules that read the frame tables hold at least three loaders each.
    if loaders < 3 * 14 {
        return Err(format!("only {loaders} loaders were compared"));
    }
    check_shader_quad()
}

/// Checks the `cr_ShaderQuad` that the epilogue of an application shader declares.
fn check_shader_quad() -> Result<(), String> {
    use iceblink::{ShaderMode, ShaderQuad};

    let source = iceblink_wgsl::effect(
        ShaderMode::PAINT,
        "fn shade(input: ShaderInput, params: Params) -> vec4<f32> { return vec4<f32>(1.0); }",
    )
    .ok_or("a paint shader does not assemble")?;
    let quad = reflected!(
        ShaderQuad,
        "cr_ShaderQuad",
        [
            bounds,
            radii,
            shapes,
            border,
            fill,
            stroke,
            paint_origin,
            clip,
            space,
            shader,
            params,
            opacity
        ]
    );
    reflect::check(&source, &[quad]).map_err(|error| format!("shader quad: {error}"))
}
