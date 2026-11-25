// Recognised shapes drawn from a shared payload: what every mark module shares.
//
// A mark item refers to its payload by count only. One draw call covers one payload kind of one
// item, and its instances walk the item's part of that kind's payload range, so `instance_index` is
// the prim's index. The whole payload stays bound, so a prim reads its neighbours past the part.
// The draw's own block identifies the item, through the remap and the chunk offsets every lane
// reads.
//
// A payload maps to the item's local space by the item's axes and origin. Distances are measured in
// a *distance space*: payload units without the screen flag, so every length scales with the
// axes, and local units with it, so a marker keeps its size under any axes.
//
// A draw paints in one of two ways. A *paint* draw writes paint times coverage times clip into the
// target, which is exact when the prims are apart. A *coverage* draw writes the coverage alone into
// a bin, and one composite later paints the item through the bin: the union of overlapping prims,
// painted once.
//
// A bin texel holds the coverage of each quarter of its pixel, one in each channel. The polylines
// write the bin first and keep the largest coverage of each quarter. Every fill then adds its
// coverage of the whole pixel to each quarter, and each quarter saturates at one. A sum is exact
// where two fills abut. The largest coverage is close where the many segments of one stroke
// overlap, and the quarters let strokes that cover different parts of a pixel cover it together.

struct MarkItem {
    bounds: cr_Bounds,
    paint: cr_PaintRef,
    paint_origin: cr_Vector2,
    // Added to every mapped payload position. A move changes this and leaves the payload as it is.
    origin: cr_Vector2,
    // The linear part of the payload-to-local map: x' = a x + c y, y' = b x + d y.
    axes: cr_Vector4,
    half_width: f32,
    discs: u32,
    boxes: u32,
    vertices: u32,
    placements: u32,
    // The first prim the item draws in its payload kind. The draw's instance range starts there.
    first: u32,
    payload: u32,
    clip: u32,
    space: u32,
    flags: u32,
    // The atlas page that holds the payload's shape cells.
    page: u32,
}

// What one draw of one item reads beside the payload.
//
// Plain vectors rather than the scalar spellings the instance structures use, because a uniform
// block aligns a nested structure to sixteen bytes.
struct MarkDraw {
    // The item's position in the lane's draw order.
    position: u32,
    // The bin page a coverage draw writes and a composite reads.
    page: u32,
    // Added to a device point to find its texel in the bin page.
    shift: vec2<f32>,
    // The device pixels the bin holds: origin then extent.
    region: vec4<f32>,
    // x: where the item's payload cells start in the cell arena.
    cells: vec4<u32>,
}

@group(1) @binding(0) var marks: texture_2d<u32>;

// One `FrameMarks`, which the arena pads to 7 texels.
fn load_mark(slot: u32) -> MarkItem {
    let base = slot * 7u;
    let t0 = cr_texel_f32(textureLoad(marks, cr_table_texel(base + 0u), 0));
    let t1 = textureLoad(marks, cr_table_texel(base + 1u), 0);
    let t2 = cr_texel_f32(textureLoad(marks, cr_table_texel(base + 2u), 0));
    let t3 = textureLoad(marks, cr_table_texel(base + 3u), 0);
    let t4 = textureLoad(marks, cr_table_texel(base + 4u), 0);
    let t5 = textureLoad(marks, cr_table_texel(base + 5u), 0);
    let t6 = textureLoad(marks, cr_table_texel(base + 6u), 0);
    return MarkItem(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_PaintRef(cr_paint_kind(t1.x), t1.y),
        cr_Vector2(bitcast<f32>(t1.z), bitcast<f32>(t1.w)),
        cr_Vector2(t2.x, t2.y),
        cr_Vector4(t2.z, t2.w, bitcast<f32>(t3.x), bitcast<f32>(t3.y)),
        bitcast<f32>(t3.z),
        t3.w,
        t4.x,
        t4.y,
        t4.z,
        t4.w,
        t5.x,
        t5.y,
        t5.z,
        t5.w,
        t6.x,
    );
}
// The draw order, indexed. A mark's instances are its prims, so its lane reads the order by
// position instead of as vertex input.
@group(1) @binding(1) var order: texture_2d<u32>;
@group(2) @binding(0) var<uniform> mark_draw: MarkDraw;

const MARK_UNION: u32 = 1u;
const MARK_SCREEN: u32 = 2u;
const MARK_SQUARE_DISCS: u32 = 4u;
const MARK_START_CAP_SHIFT: u32 = 8u;
const MARK_END_CAP_SHIFT: u32 = 10u;
const CAP_BUTT: u32 = 0u;
const CAP_SQUARE: u32 = 1u;
const CAP_ROUND: u32 = 2u;

struct MarkVarying {
    @builtin(position) position: vec4<f32>,
    // The point in the item's own space, with the chunk offset added.
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) slot: u32,
    @location(2) @interpolate(flat) shift: vec2<f32>,
    // The prim's index in its payload kind.
    @location(3) @interpolate(flat) prim: u32,
    // The polyline caps: start in bits 0 and 1, end in bits 2 and 3.
    @location(4) @interpolate(flat) caps: u32,
    // The point in distance space, with no chunk offset.
    @location(5) point: vec2<f32>,
}

// The item of a draw: its arena slot, and the offset it is drawn moved by.
struct MarkRef {
    slot: u32,
    shift: cr_Vector2,
}

// One `MarkRef`, which the table pads to a whole texel.
fn item_of(position: u32) -> MarkRef {
    let t0 = textureLoad(order, cr_table_texel(position), 0);
    return MarkRef(t0.x, cr_Vector2(bitcast<f32>(t0.y), bitcast<f32>(t0.z)));
}

// The shift one entry carries, as a vector.
fn shift_of(found: MarkRef) -> vec2<f32> {
    return vec2<f32>(found.shift.x, found.shift.y);
}

// The length in local units of the shortest device pixel: one over the smallest singular value of
// the space's linear part.
fn margin_local(space: u32) -> f32 {
    let matrix = cr_load_spatial(space);
    return margin_of(matrix[0].xy, matrix[1].xy);
}

// One over the smallest singular value of the matrix with columns `x` and `y`, at most 1e4.
fn margin_of(x: vec2<f32>, y: vec2<f32>) -> f32 {
    let p = dot(x, x);
    let q = dot(y, y);
    let r = dot(x, y);
    let spread = sqrt(max(0.25 * (p - q) * (p - q) + r * r, 0.0));
    let smallest = sqrt(max(0.5 * (p + q) - spread, 1e-12));
    return min(1.0 / smallest, 1e4);
}

// The item's axes as a matrix.
fn mark_axes(item: MarkItem) -> mat2x2<f32> {
    return mat2x2<f32>(item.axes.x, item.axes.y, item.axes.z, item.axes.w);
}

fn is_screen(item: MarkItem) -> bool {
    return (item.flags & MARK_SCREEN) != 0u;
}

// A payload position in distance space.
fn to_space(item: MarkItem, p: vec2<f32>) -> vec2<f32> {
    if is_screen(item) {
        return mark_axes(item) * p + vec2<f32>(item.origin.x, item.origin.y);
    }
    return p;
}

// A point in distance space, in the item's local space.
fn to_local(item: MarkItem, q: vec2<f32>) -> vec2<f32> {
    if is_screen(item) {
        return q;
    }
    return mark_axes(item) * q + vec2<f32>(item.origin.x, item.origin.y);
}

// The length in distance units of the shortest device pixel.
fn margin_space(item: MarkItem) -> f32 {
    if is_screen(item) {
        return margin_local(item.space);
    }
    let matrix = cr_load_spatial(item.space);
    let linear = mat2x2<f32>(matrix[0].xy, matrix[1].xy) * mark_axes(item);
    return margin_of(linear[0], linear[1]);
}

// A point in the item's space, on the device and moved by `shift`, in the target's clip space.
fn to_target(local: vec2<f32>, space: u32, shift: vec2<f32>) -> vec4<f32> {
    let world = cr_load_spatial(space) * vec4<f32>(local, 0.0, 1.0);
    let device = world.xy / world.w + shift;
    let texel = (device - cr_globals.origin.xy) * cr_globals.viewport.zw;
    return vec4<f32>(
        texel.x / cr_globals.viewport.x * 2.0 - 1.0,
        1.0 - texel.y / cr_globals.viewport.y * 2.0,
        0.0,
        1.0,
    );
}

// A point in the item's space, in the clip space of the bin page this draw writes.
fn to_page(local: vec2<f32>, space: u32) -> vec4<f32> {
    return to_target(local, space, mark_draw.shift);
}

// A signed distance in payload units, negative inside, and its gradient.
struct Distance {
    d: f32,
    gradient: vec2<f32>,
    // The width of the shape behind the edge, against the gradient: a band, such as a ring or a
    // stroke, ends at a far edge this far from the near one. Zero for a shape with no far edge.
    band: f32,
}

// The coverage of `distance`: the area of the pixel inside the edge.
//
// `across` and `down` are how far one device pixel moves the payload point. The gradient through
// them is the edge's normal on the screen, so the coverage holds under any scale or turn.
//
// A band thinner than a pixel can end inside the pixel on both sides, and the area past its far
// edge is removed, so a hairline covers its own width.
fn sdf_coverage(distance: Distance, across: vec2<f32>, down: vec2<f32>) -> f32 {
    let normal = vec2<f32>(dot(distance.gradient, across), dot(distance.gradient, down));
    let step = max(length(normal), 1e-6);
    let along = abs(normal) / step;
    let a = max(along.x, along.y);
    let b = min(along.x, along.y);
    let inside = -distance.d / step;
    let near = edge_coverage(inside, a, b);
    if distance.band <= 0.0 {
        return near;
    }
    return max(near - edge_coverage(inside - distance.band / step, a, b), 0.0);
}

// The area of a unit pixel on the inner side of a straight edge `inside` pixels from its centre,
// for an edge whose unit normal has the components `a` and `b`, `a` the larger.
//
// The pixel's projection onto the normal is a trapezoid, flat over `a - b` and sloped over `b` at
// each end, so its area up to `inside` is exact for a straight edge at any angle.
fn edge_coverage(inside: f32, a: f32, b: f32) -> f32 {
    let outer = 0.5 * (a + b);
    let inner = 0.5 * (a - b);
    if inside <= -outer {
        return 0.0;
    }
    if inside >= outer {
        return 1.0;
    }
    if b < 1e-4 {
        return saturate(0.5 + inside / a);
    }
    if inside < -inner {
        let t = inside + outer;
        return t * t / (2.0 * a * b);
    }
    if inside > inner {
        let t = outer - inside;
        return 1.0 - t * t / (2.0 * a * b);
    }
    return 0.5 + inside / a;
}

// The larger of two distances, with the gradient of that one.
fn distance_max(one: Distance, two: Distance) -> Distance {
    if two.d > one.d {
        return two;
    }
    return one;
}

// The smaller of two distances, with the gradient of that one.
fn distance_min(one: Distance, two: Distance) -> Distance {
    if two.d < one.d {
        return two;
    }
    return one;
}

// The distance from a disc of `radius` around `centre`, outward from it.
fn radial(point: vec2<f32>, centre: vec2<f32>, radius: f32) -> Distance {
    let along = point - centre;
    let length_along = length(along);
    var out: Distance;
    out.d = length_along - radius;
    out.gradient = select(vec2<f32>(1.0, 0.0), along / length_along, length_along > 0.0);
    out.band = 0.0;
    return out;
}

// The varying for the point `q` in distance space of one prim.
fn mark_point(q: vec2<f32>, item: MarkItem, shift: vec2<f32>, coverage: bool) -> MarkVarying {
    let local = to_local(item, q) + shift;
    var out: MarkVarying;
    if coverage {
        out.position = to_page(local, item.space);
    } else {
        out.position = to_target(local, item.space, vec2<f32>(0.0));
    }
    out.local = local;
    out.shift = shift;
    out.caps = 0u;
    out.point = q;
    return out;
}

// The varying for one corner of a quad over `bounds` (x0 y0 x1 y1, distance space) of one prim.
fn mark_corner(
    vertex: u32,
    bounds: vec4<f32>,
    item: MarkItem,
    shift: vec2<f32>,
    coverage: bool,
) -> MarkVarying {
    let corner = cr_unit_corner(vertex);
    return mark_point(bounds.xy + corner * (bounds.zw - bounds.xy), item, shift, coverage);
}

// Where a fragment's sample lies in distance space.
fn payload_point(in: MarkVarying) -> vec2<f32> {
    return in.point;
}

// Where a paint draw's pixel centre lies in distance space, found from the pixel and the item
// alone, so every prim that draws the pixel finds the same point.
fn pixel_point(in: MarkVarying, item: MarkItem) -> vec2<f32> {
    let local = cr_space_point(cr_device_position(in.position.xy), item.space) - in.shift;
    if is_screen(item) {
        return local;
    }
    let axes = mark_axes(item);
    let inverse = mat2x2<f32>(axes[1].y, -axes[0].y, -axes[1].x, axes[0].x) * (1.0 / determinant(axes));
    return inverse * (local - vec2<f32>(item.origin.x, item.origin.y));
}

// The premultiplied colour a paint draw writes for `coverage`.
fn mark_paint(in: MarkVarying, item: MarkItem, coverage: f32) -> vec4<f32> {
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), item.clip);
    if clip <= 0.0 || coverage <= 0.0 {
        return vec4<f32>(0.0);
    }
    let point = in.local - in.shift;
    let origin = vec2<f32>(item.paint_origin.x, item.paint_origin.y);
    return cr_paint_color(item.paint, point, origin) * coverage * clip;
}

// What a coverage draw writes into its bin, one quarter of the pixel in each channel: nothing
// outside the bin's region.
fn mark_bin(in: MarkVarying, quarters: vec4<f32>) -> vec4<f32> {
    return select(vec4<f32>(0.0), quarters, in_bin(in.position.xy));
}

// Whether the bin texel at `texel` lies in the bin's region.
fn in_bin(texel: vec2<f32>) -> bool {
    let device = texel - mark_draw.shift;
    let region = mark_draw.region;
    return device.x >= region.x && device.y >= region.y
        && device.x < region.x + region.z && device.y < region.y + region.w;
}

// Whether a payload vertex is a run separator.
fn is_separator(vertex: vec2<f32>) -> bool {
    let bits = bitcast<vec2<u32>>(vertex) & vec2<u32>(0x7fffffffu);
    return bits.x > 0x7f800000u || bits.y > 0x7f800000u;
}
