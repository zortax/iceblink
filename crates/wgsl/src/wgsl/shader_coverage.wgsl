// A coverage shader: the application shapes the box, and the renderer fills it with the quad's
// paints. The border is the ring between the shape and the same shape evaluated on the box shrunk
// by the border widths.

@fragment
fn cr_fs_shader_coverage(in: cr_ShaderQuadVarying) -> @location(0) vec4<f32> {
    let quad = cr_load_shader_quad(in.instance);
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), quad.clip);
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }
    let point = in.origin + in.local;
    let paint_origin = vec2<f32>(quad.paint_origin.x, quad.paint_origin.y);
    let input = cr_shader_input(quad, in.local);
    let outer = saturate(coverage(input, cr_shader_params.user));
    if outer <= 0.0 {
        return vec4<f32>(0.0);
    }

    let inset = vec2<f32>(
        0.5 * (quad.border.left + quad.border.right),
        0.5 * (quad.border.top + quad.border.bottom),
    );
    var inner = outer;
    if inset.x > 0.0 || inset.y > 0.0 {
        var shrunk = input;
        shrunk.size = max(input.size - 2.0 * inset, vec2<f32>(1.0e-4));
        shrunk.local = input.local - inset;
        shrunk.uv = shrunk.local / shrunk.size;
        inner = saturate(coverage(shrunk, cr_shader_params.user));
    }

    // The inner shape lies inside the outer one, so the ring is a sum.
    let background = cr_paint_color(quad.fill, point, paint_origin) * inner;
    let border = cr_paint_color(quad.stroke, point, paint_origin) * max(outer - inner, 0.0);
    return (background + border) * clip * saturate(quad.opacity);
}
