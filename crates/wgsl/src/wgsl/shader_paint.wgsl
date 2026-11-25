// A paint shader: the application shades the colour, and the renderer applies the rounded box.

@fragment
fn cr_fs_shader_paint(in: cr_ShaderQuadVarying) -> @location(0) vec4<f32> {
    let quad = cr_load_shader_quad(in.instance);
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), quad.clip);
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }
    let input = cr_shader_input(quad, in.local);
    let color = clamp(shade(input, cr_shader_params.user), vec4<f32>(0.0), vec4<f32>(1.0));
    let bounds = cr_bounds_from(quad.bounds, in.origin);
    let shape = coverage_of(cr_quad_sdf(in.local, bounds, quad.radii, quad.shapes));
    return color * shape * clip * saturate(quad.opacity);
}
