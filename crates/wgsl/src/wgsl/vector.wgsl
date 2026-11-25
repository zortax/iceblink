// Compositing a rasterised vector pass into the target at its point in paint order.
//
// One scratch texel is one device pixel, so the read is a `textureLoad` with no sampler. The
// scratch holds straight colour, which this premultiplies.

struct VectorInstance {
    // The quad, in device pixels: origin then extent.
    bounds: vec4<f32>,
    // xy: the scratch texel the quad's origin reads. zw: unused.
    source: vec4<f32>,
    // x: the bits of the clip slot this instance binds. yzw: unused.
    control: vec4<f32>,
}

@group(1) @binding(0) var vector_instances: texture_2d<u32>;

// One `VectorInstance`, which is three texels.
fn load_vector(index: u32) -> VectorInstance {
    let base = index * 3u;
    return VectorInstance(
        cr_texel_f32(textureLoad(vector_instances, cr_table_texel(base + 0u), 0)),
        cr_texel_f32(textureLoad(vector_instances, cr_table_texel(base + 1u), 0)),
        cr_texel_f32(textureLoad(vector_instances, cr_table_texel(base + 2u), 0)),
    );
}
@group(1) @binding(1) var vector_scratch: texture_2d<f32>;

struct VectorVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) instance: u32,
}

@vertex
fn vs_vector(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VectorVarying {
    let record = load_vector(instance);
    let device = record.bounds.xy + cr_unit_corner(vertex) * record.bounds.zw;
    var out: VectorVarying;
    out.position = cr_device_to_target(device);
    out.instance = instance;
    return out;
}

@fragment
fn fs_vector(in: VectorVarying) -> @location(0) vec4<f32> {
    let record = load_vector(in.instance);
    let device = cr_device_position(in.position.xy);
    let coverage = cr_clip_coverage(device, bitcast<u32>(record.control.x));
    if coverage <= 0.0 {
        return vec4<f32>(0.0);
    }
    let texel = vec2<i32>(floor(device - record.bounds.xy + record.source.xy));
    let straight = textureLoad(vector_scratch, texel, 0);
    let premultiplied = vec4<f32>(straight.rgb * straight.a, straight.a);
    return premultiplied * coverage;
}
