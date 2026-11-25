// Placements of a mark payload: shapes rasterised at quarter-pixel phases, read from atlas cells.
//
// A placement is `[x, y, shape, 0]`, and an item's cells are sixteen per shape, indexed
// `shape * 16 + phase_y * 4 + phase_x`. A cell is `[x, y, w, h]` in texels of the item's page,
// then its offset from the anchor's pixel.
//
// The phase is taken from the anchor on the device, with the drawn offset in it, so a moved item
// draws the cell of its new phase. The cell is drawn on whole device pixels and read one texel at a
// time.

@group(2) @binding(1) var mark_placements: texture_2d<u32>;
@group(2) @binding(2) var mark_cells: texture_2d<u32>;

fn load_mark_placement(index: u32) -> vec4<u32> {
    return textureLoad(mark_placements, cr_table_texel(index), 0);
}

// One cell, which the arena pads to two texels: the rectangle, then the offset.
fn mark_cell(index: u32) -> array<vec4<u32>, 2> {
    return array<vec4<u32>, 2>(
        textureLoad(mark_cells, cr_table_texel(index * 2u), 0),
        textureLoad(mark_cells, cr_table_texel(index * 2u + 1u), 0),
    );
}
@group(3) @binding(0) var atlas: texture_2d<f32>;

struct GlyphVarying {
    @builtin(position) position: vec4<f32>,
    // The point in the item's own space, with the chunk offset added.
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) slot: u32,
    @location(2) @interpolate(flat) shift: vec2<f32>,
    // The device pixel the cell starts at.
    @location(3) @interpolate(flat) corner: vec2<f32>,
    // The cell in atlas texels: origin then extent.
    @location(4) @interpolate(flat) cell: vec4<i32>,
}

// The pixel a device coordinate is drawn from, and its quarter-pixel phase, per axis.
//
// Quantised to a quarter pixel first and split after.
fn glyph_phase(device: vec2<f32>) -> vec4<f32> {
    let quantised = floor(4.0 * device + vec2<f32>(0.5));
    let phase = quantised - 4.0 * floor(quantised / 4.0);
    return vec4<f32>((quantised - phase) / 4.0, phase);
}

// A device point in the clip space of the target, or of the bin page with `shift` added.
fn device_to_clip(device: vec2<f32>, shift: vec2<f32>) -> vec4<f32> {
    let texel = (device + shift - cr_globals.origin.xy) * cr_globals.viewport.zw;
    return vec4<f32>(
        texel.x / cr_globals.viewport.x * 2.0 - 1.0,
        1.0 - texel.y / cr_globals.viewport.y * 2.0,
        0.0,
        1.0,
    );
}

fn glyph_corner(vertex: u32, instance: u32, coverage: bool) -> GlyphVarying {
    let found = item_of(mark_draw.position);
    let item = load_mark(found.slot);
    let record = load_mark_placement(instance);
    let anchor = mark_axes(item) * bitcast<vec2<f32>>(record.xy)
        + vec2<f32>(item.origin.x, item.origin.y) + shift_of(found);
    let matrix = cr_load_spatial(item.space);
    let world = matrix * vec4<f32>(anchor, 0.0, 1.0);
    let split = glyph_phase(world.xy / world.w);
    let phase = u32(split.z) + 4u * u32(split.w);
    let entry = mark_cell(mark_draw.cells.x + 16u * record.z + phase);
    let size = entry[0].zw;
    let corner = split.xy + vec2<f32>(bitcast<vec2<i32>>(entry[1].xy));
    var device = corner + cr_unit_corner(vertex) * vec2<f32>(size);
    if !coverage {
        // Out to whole texels of the target, which hold two device pixels at half resolution.
        let scale = cr_globals.viewport.zw;
        let origin = cr_globals.origin.xy;
        let low = floor((corner - origin) * scale) / scale + origin;
        let high = ceil((corner + vec2<f32>(size) - origin) * scale) / scale + origin;
        device = low + cr_unit_corner(vertex) * (high - low);
    }
    // Back into the item's space, for the paint. The space is affine on this route.
    let linear = mat2x2<f32>(matrix[0].xy, matrix[1].xy);
    let determinant = linear[0].x * linear[1].y - linear[1].x * linear[0].y;
    let inverse = mat2x2<f32>(
        vec2<f32>(linear[1].y, -linear[0].y),
        vec2<f32>(-linear[1].x, linear[0].x),
    ) * (1.0 / determinant);
    var out: GlyphVarying;
    if coverage {
        out.position = device_to_clip(device, mark_draw.shift);
    } else {
        out.position = device_to_clip(device, vec2<f32>(0.0));
    }
    out.local = inverse * (device - matrix[3].xy);
    out.slot = found.slot;
    out.shift = shift_of(found);
    out.corner = corner;
    out.cell = vec4<i32>(entry[0]);
    return out;
}

// The coverage of the cell at the device pixel whose centre is `device`, and nothing outside it.
fn cell_coverage(in: GlyphVarying, device: vec2<f32>) -> f32 {
    let texel = vec2<i32>(floor(device - in.corner));
    if texel.x < 0 || texel.y < 0 || texel.x >= in.cell.z || texel.y >= in.cell.w {
        return 0.0;
    }
    return textureLoad(atlas, in.cell.xy + texel, 0).r;
}

@vertex
fn vs_glyph_paint(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> GlyphVarying {
    return glyph_corner(vertex, instance, false);
}

@vertex
fn vs_glyph_coverage(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> GlyphVarying {
    return glyph_corner(vertex, instance, true);
}

@fragment
fn fs_glyph_paint(in: GlyphVarying) -> @location(0) vec4<f32> {
    let item = load_mark(in.slot);
    let device = cr_device_position(in.position.xy);
    var coverage = 0.0;
    if cr_globals.viewport.z >= 1.0 {
        coverage = cell_coverage(in, floor(device) + vec2<f32>(0.5));
    } else {
        // The four device pixels whose centres this texel's footprint holds.
        let first = floor(device - vec2<f32>(0.5)) + vec2<f32>(0.5);
        coverage = 0.25 * (cell_coverage(in, first) + cell_coverage(in, first + vec2<f32>(1.0, 0.0))
            + cell_coverage(in, first + vec2<f32>(0.0, 1.0))
            + cell_coverage(in, first + vec2<f32>(1.0, 1.0)));
    }
    let clip = cr_clip_coverage(device, item.clip);
    if clip <= 0.0 || coverage <= 0.0 {
        return vec4<f32>(0.0);
    }
    let point = in.local - in.shift;
    let origin = vec2<f32>(item.paint_origin.x, item.paint_origin.y);
    return cr_paint_color(item.paint, point, origin) * coverage * clip;
}

@fragment
fn fs_glyph_coverage(in: GlyphVarying) -> @location(0) vec4<f32> {
    let coverage = cell_coverage(in, in.position.xy - mark_draw.shift);
    return select(vec4<f32>(0.0), vec4<f32>(coverage), in_bin(in.position.xy));
}
