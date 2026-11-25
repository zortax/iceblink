// The renderer's half of an application shader that draws a rectangle: the instance, the
// parameter block, the vertex stage, and the input the shader is called with. The instance goes
// through the same arena, draw order and offsets as a quad.

struct cr_ShaderQuad {
    bounds: cr_Bounds,
    radii: cr_Radii,
    shapes: cr_Shapes,
    border: cr_Edges,
    // Read by a coverage shader. A paint shader reads neither paint.
    fill: cr_PaintRef,
    stroke: cr_PaintRef,
    paint_origin: cr_Vector2,
    clip: u32,
    space: u32,
    // The shader slot. Read on the host to split batches.
    shader: u32,
    // The parameter slot. The block is bound beside the draw, since every instance shares it.
    params: u32,
    opacity: f32,
}

@group(1) @binding(0) var cr_shader_quads: texture_2d<u32>;

// One `cr_ShaderQuad`, which the arena pads to 8 texels.
fn cr_load_shader_quad(slot: u32) -> cr_ShaderQuad {
    let base = slot * 8u;
    let t0 = cr_texel_f32(textureLoad(cr_shader_quads, cr_table_texel(base + 0u), 0));
    let t1 = cr_texel_f32(textureLoad(cr_shader_quads, cr_table_texel(base + 1u), 0));
    let t2 = cr_texel_f32(textureLoad(cr_shader_quads, cr_table_texel(base + 2u), 0));
    let t3 = cr_texel_f32(textureLoad(cr_shader_quads, cr_table_texel(base + 3u), 0));
    let t4 = cr_texel_f32(textureLoad(cr_shader_quads, cr_table_texel(base + 4u), 0));
    let t5 = textureLoad(cr_shader_quads, cr_table_texel(base + 5u), 0);
    let t6 = textureLoad(cr_shader_quads, cr_table_texel(base + 6u), 0);
    let t7 = textureLoad(cr_shader_quads, cr_table_texel(base + 7u), 0);
    return cr_ShaderQuad(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_Radii(t1.x, t1.y, t1.z, t1.w, t2.x, t2.y, t2.z, t2.w),
        cr_Shapes(t3.x, t3.y, t3.z, t3.w),
        cr_Edges(t4.x, t4.y, t4.z, t4.w),
        cr_PaintRef(cr_paint_kind(t5.x), t5.y),
        cr_PaintRef(cr_paint_kind(t5.z), t5.w),
        cr_Vector2(bitcast<f32>(t6.x), bitcast<f32>(t6.y)),
        t6.z,
        t6.w,
        t7.x,
        t7.y,
        bitcast<f32>(t7.z),
    );
}

// One parameter block: the pointer fields, then the shader's own `Params`, declared above.
struct cr_ShaderParams {
    // The pointer, from the bounds origin in local units.
    pointer: vec2<f32>,
    // One while the pointer is over the item.
    hovered: f32,
    // Puts the shader's half on a sixteen-byte boundary.
    reserved: f32,
    // Sized to the whole block whatever the shader declares, as WebGL 2 needs a multiple of sixteen
    // bytes.
    @size(64) user: Params,
}

@group(2) @binding(0) var<uniform> cr_shader_params: cr_ShaderParams;

struct cr_ShaderQuadVarying {
    @builtin(position) position: vec4<f32>,
    // The point, from the quad's origin in the space it was encoded in.
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) instance: u32,
    // The quad's origin in that space.
    @location(2) @interpolate(flat) origin: vec2<f32>,
}

@vertex
fn cr_vs_shader_quad(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> cr_ShaderQuadVarying {
    let quad = cr_load_shader_quad(slot);
    let local = cr_inflated_corner(vertex, quad.bounds) + shift;
    var out: cr_ShaderQuadVarying;
    out.position = cr_to_clip_position(local, quad.space);
    out.local = cr_inflated_offset(vertex, quad.bounds);
    out.instance = slot;
    out.origin = cr_bounds_origin(quad.bounds);
    return out;
}

// The shader's input at `local` from the quad's origin.
fn cr_shader_input(quad: cr_ShaderQuad, local: vec2<f32>) -> ShaderInput {
    let size = max(cr_bounds_size(quad.bounds), vec2<f32>(1.0e-4));
    var input: ShaderInput;
    input.local = local;
    input.uv = input.local / size;
    input.size = size;
    input.scale = cr_globals.frame.z;
    input.time = cr_globals.frame.x;
    input.delta = cr_globals.frame.y;
    input.pointer = cr_shader_params.pointer;
    input.hovered = cr_shader_params.hovered;
    return input;
}
