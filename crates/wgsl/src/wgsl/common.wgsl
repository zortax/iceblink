// The declarations every pipeline shares: the frame's globals, the frame tables every instance
// indexes, and the frame kinds as packed scalars.
//
// Every field of an instance structure is a four-byte scalar. A `vec4<f32>` member would carry a
// sixteen-byte alignment the packed Rust structures do not have. The shader reflection check
// compares these declarations with the Rust ones field by field.

struct cr_Globals {
    // xy: the target's extent in texels. zw: texels per device pixel, a half in a half-resolution
    // group target.
    viewport: vec4<f32>,
    // The four gamma-correction coefficients coverage is corrected with.
    gamma_ratios: vec4<f32>,
    // x: contrast enhancement for single-channel coverage. y: the same for per-channel coverage.
    // z: non-zero when the display's stripes run blue to red.
    text: vec4<f32>,
    // x: the shader clock's time. y: its delta. z: the output's scale factor.
    frame: vec4<f32>,
    // xy: the device pixel the target's first texel holds.
    origin: vec4<f32>,
}

// A rectangle: origin then extent.
struct cr_Bounds {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

// Per-corner elliptical radii, clockwise from the top left.
struct cr_Radii {
    tl_x: f32,
    tl_y: f32,
    tr_x: f32,
    tr_y: f32,
    br_x: f32,
    br_y: f32,
    bl_x: f32,
    bl_y: f32,
}

// Per-corner superellipse exponents, clockwise from the top left.
struct cr_Shapes {
    tl: f32,
    tr: f32,
    br: f32,
    bl: f32,
}

// Premultiplied, gamma-encoded sRGB.
struct cr_Rgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

// Widths of the four sides.
struct cr_Edges {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

// A paint kind and a frame paint slot.
struct cr_PaintRef {
    kind: u32,
    index: u32,
}

struct cr_Vector4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

struct cr_Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

// Spelled as scalars so it carries no wider alignment than the packed structures holding it.
struct cr_Vector2 {
    x: f32,
    y: f32,
}

// A rectangle of an atlas page, in texels.
struct cr_Tile {
    page: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

// One rounded-rectangle test of a clip.
struct cr_Rounded {
    rect: cr_Bounds,
    radii: cr_Radii,
    shapes: cr_Shapes,
}

// One clip record of the clip table.
struct cr_Clip {
    // The device box the clip rejects outside of.
    aabb: cr_Bounds,
    // Rounded tests in device space, or in the kept space where `kept_tests` says so.
    first: cr_Rounded,
    second: cr_Rounded,
    // The kept space's box, in its coordinates.
    kept_aabb: cr_Bounds,
    count: u32,
    space: u32,
    kept_tests: u32,
    // The index of the first general link plus one, or zero without general links.
    links: u32,
}

// One general link of the link table, evaluated in its own space.
struct cr_ClipLink {
    // 0 a rounded rectangle, 1 an alpha mask, 2 a colour mask.
    kind: u32,
    space: u32,
    // 1 for a luminance mask.
    mode: u32,
    // On a clip's first link, how many links the clip has.
    count: u32,
    rect: cr_Bounds,
    radii: cr_Radii,
    // The exponents of a rounded link, or the mask's cell in its mask texture.
    extra: cr_Vector4,
}

// One paint.
struct cr_Paint {
    // 0 nothing, 1 solid, 2 gradient, 4 one colour per border side.
    kind: u32,
    // 0 linear, 1 radial, 2 conic.
    gradient: u32,
    // The stop encoding: 0 sRGB, 1 Oklab, 2 linear sRGB.
    space: u32,
    // 1 when the ramp repeats.
    flags: u32,
    // Linear: start then end. Radial: centre then radii. Conic: centre, then the start angle in w.
    geometry: cr_Vector4,
    color: cr_Rgba,
    stop_start: u32,
    stop_count: u32,
    pad0: u32,
    pad1: u32,
}

// One stop of a ramp: premultiplied components of the ramp's encoding, then its offset.
struct cr_Stop {
    color: cr_Vector4,
    offset: f32,
    pad: cr_Vector3,
}

@group(0) @binding(0) var<uniform> cr_globals: cr_Globals;
// The frame tables are `rgba32uint` textures read one texel at a time, because a GL 3.3 context has
// no storage buffers. Every record is a whole number of texels.
@group(0) @binding(1) var cr_clips: texture_2d<u32>;
@group(0) @binding(2) var cr_paints: texture_2d<u32>;
@group(0) @binding(3) var cr_stops: texture_2d<u32>;
@group(0) @binding(4) var cr_spatial: texture_2d<u32>;
@group(0) @binding(5) var cr_clip_links: texture_2d<u32>;
@group(0) @binding(6) var cr_clip_alpha_masks: texture_2d<f32>;
@group(0) @binding(7) var cr_clip_color_masks: texture_2d<f32>;

// How many texels wide every table is. A test compares it with the Rust side.
const cr_TABLE_TEXELS_WIDE: u32 = 256u;

// The frame clip slot of an instance drawn without a clip.
const cr_NO_CLIP: u32 = 0xffffffffu;

// What a clip's kept space holds when no link is kept in a space of its own.
const cr_NO_SPACE: u32 = 0xffffffffu;

fn cr_table_texel(index: u32) -> vec2<i32> {
    return vec2<i32>(i32(index % cr_TABLE_TEXELS_WIDE), i32(index / cr_TABLE_TEXELS_WIDE));
}

fn cr_texel_f32(t: vec4<u32>) -> vec4<f32> {
    return bitcast<vec4<f32>>(t);
}

// A space spans eight texels: the world matrix, the inverse of its plane homography, and a row
// of flags.
const cr_SPACE_TEXELS: u32 = 8u;

fn cr_load_spatial(id: u32) -> mat4x4<f32> {
    let base = id * cr_SPACE_TEXELS;
    return mat4x4<f32>(
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 0u), 0)),
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 1u), 0)),
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 2u), 0)),
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 3u), 0)),
    );
}

// The map from a device point to the space's plane, as a homography on `(x, y, 1)`.
fn cr_space_inverse(id: u32) -> mat3x3<f32> {
    let base = id * cr_SPACE_TEXELS + 4u;
    return mat3x3<f32>(
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 0u), 0)).xyz,
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 1u), 0)).xyz,
        cr_texel_f32(textureLoad(cr_spatial, cr_table_texel(base + 2u), 0)).xyz,
    );
}

// The point of a space's plane that maps to a device point.
fn cr_space_point(device: vec2<f32>, id: u32) -> vec2<f32> {
    let mapped = cr_space_inverse(id) * vec3<f32>(device, 1.0);
    return mapped.xy / mapped.z;
}

fn cr_load_paint(id: u32) -> cr_Paint {
    let base = id * 4u;
    let t0 = textureLoad(cr_paints, cr_table_texel(base + 0u), 0);
    let g = cr_texel_f32(textureLoad(cr_paints, cr_table_texel(base + 1u), 0));
    let c = cr_texel_f32(textureLoad(cr_paints, cr_table_texel(base + 2u), 0));
    let t3 = textureLoad(cr_paints, cr_table_texel(base + 3u), 0);
    return cr_Paint(
        t0.x, t0.y, t0.z, t0.w,
        cr_Vector4(g.x, g.y, g.z, g.w),
        cr_Rgba(c.x, c.y, c.z, c.w),
        t3.x, t3.y, t3.z, t3.w,
    );
}

// A clip spans eleven texels: the device box, two rounded tests of four texels each, the kept
// box, then the scalars. Each field is fetched where it is used, so a clip that is a box reads
// one texel.
const cr_CLIP_TEXELS: u32 = 11u;

fn cr_clip_aabb(id: u32) -> cr_Bounds {
    let t = cr_texel_f32(textureLoad(cr_clips, cr_table_texel(id * cr_CLIP_TEXELS + 0u), 0));
    return cr_Bounds(t.x, t.y, t.z, t.w);
}

// Rounded test `which`, zero outermost.
fn cr_clip_rounded(id: u32, which: u32) -> cr_Rounded {
    let base = id * cr_CLIP_TEXELS + 1u + which * 4u;
    let r = cr_texel_f32(textureLoad(cr_clips, cr_table_texel(base + 0u), 0));
    let lo = cr_texel_f32(textureLoad(cr_clips, cr_table_texel(base + 1u), 0));
    let hi = cr_texel_f32(textureLoad(cr_clips, cr_table_texel(base + 2u), 0));
    let s = cr_texel_f32(textureLoad(cr_clips, cr_table_texel(base + 3u), 0));
    return cr_Rounded(
        cr_Bounds(r.x, r.y, r.z, r.w),
        cr_Radii(lo.x, lo.y, lo.z, lo.w, hi.x, hi.y, hi.z, hi.w),
        cr_Shapes(s.x, s.y, s.z, s.w),
    );
}

fn cr_clip_kept_aabb(id: u32) -> cr_Bounds {
    let t = cr_texel_f32(textureLoad(cr_clips, cr_table_texel(id * cr_CLIP_TEXELS + 9u), 0));
    return cr_Bounds(t.x, t.y, t.z, t.w);
}

// The scalars: the count of rounded tests, the kept space, which tests are kept, and the index of
// the first general link plus one.
fn cr_clip_scalars(id: u32) -> vec4<u32> {
    return textureLoad(cr_clips, cr_table_texel(id * cr_CLIP_TEXELS + 10u), 0);
}

fn cr_clip_count(id: u32) -> u32 {
    return cr_clip_scalars(id).x;
}

fn cr_clip_space(id: u32) -> u32 {
    return cr_clip_scalars(id).y;
}

fn cr_load_stop(id: u32) -> cr_Stop {
    let base = id * 2u;
    let c = cr_texel_f32(textureLoad(cr_stops, cr_table_texel(base + 0u), 0));
    let r = cr_texel_f32(textureLoad(cr_stops, cr_table_texel(base + 1u), 0));
    return cr_Stop(cr_Vector4(c.x, c.y, c.z, c.w), r.x, cr_Vector3(r.y, r.z, r.w));
}

const cr_PAINT_NONE: u32 = 0u;
const cr_PAINT_SOLID: u32 = 1u;
const cr_PAINT_GRADIENT: u32 = 2u;
const cr_PAINT_SIDES: u32 = 4u;

const cr_SPACE_SRGB: u32 = 0u;
const cr_SPACE_OKLAB: u32 = 1u;
const cr_SPACE_LINEAR_SRGB: u32 = 2u;

const cr_M_PI: f32 = 3.141592653589793;

fn cr_bounds_origin(b: cr_Bounds) -> vec2<f32> {
    return vec2<f32>(b.x, b.y);
}

fn cr_bounds_size(b: cr_Bounds) -> vec2<f32> {
    return vec2<f32>(b.w, b.h);
}

// The same bounds, measured from `origin`.
fn cr_bounds_from(b: cr_Bounds, origin: vec2<f32>) -> cr_Bounds {
    return cr_Bounds(b.x - origin.x, b.y - origin.y, b.w, b.h);
}

fn cr_bounds_of(b: cr_Bounds) -> vec4<f32> {
    return vec4<f32>(b.x, b.y, b.w, b.h);
}

fn cr_bounds_from_vec4(v: vec4<f32>) -> cr_Bounds {
    return cr_Bounds(v.x, v.y, v.z, v.w);
}

fn cr_radii_lo(r: cr_Radii) -> vec4<f32> {
    return vec4<f32>(r.tl_x, r.tl_y, r.tr_x, r.tr_y);
}

fn cr_radii_hi(r: cr_Radii) -> vec4<f32> {
    return vec4<f32>(r.br_x, r.br_y, r.bl_x, r.bl_y);
}

fn cr_radii_from_vec4(lo: vec4<f32>, hi: vec4<f32>) -> cr_Radii {
    return cr_Radii(lo.x, lo.y, lo.z, lo.w, hi.x, hi.y, hi.z, hi.w);
}

fn cr_shapes_of(s: cr_Shapes) -> vec4<f32> {
    return vec4<f32>(s.tl, s.tr, s.br, s.bl);
}

fn cr_shapes_from_vec4(v: vec4<f32>) -> cr_Shapes {
    return cr_Shapes(v.x, v.y, v.z, v.w);
}

fn cr_rgba_of(c: cr_Rgba) -> vec4<f32> {
    return vec4<f32>(c.r, c.g, c.b, c.a);
}

fn cr_tile_rect(t: cr_Tile) -> vec4<f32> {
    return vec4<f32>(f32(t.x), f32(t.y), f32(t.w), f32(t.h));
}

fn cr_vector4_of(v: cr_Vector4) -> vec4<f32> {
    return vec4<f32>(v.x, v.y, v.z, v.w);
}

fn cr_vector2_of(v: cr_Vector2) -> vec2<f32> {
    return vec2<f32>(v.x, v.y);
}

// The kind of a packed `cr_PaintRef`, whose kind byte is followed by three padding bytes.
fn cr_paint_kind(word: u32) -> u32 {
    return word & 0xffu;
}

// The four corners of the unit square, in triangle-strip order.
fn cr_unit_corner(vertex: u32) -> vec2<f32> {
    return vec2<f32>(f32(vertex & 1u), 0.5 * f32(vertex & 2u));
}

// A point of a space, projected into the current target's clip space.
fn cr_to_clip_position(point: vec2<f32>, spatial_id: u32) -> vec4<f32> {
    let world = cr_load_spatial(spatial_id) * vec4<f32>(point, 0.0, 1.0);
    let texel = (world.xy - cr_globals.origin.xy * world.w) * cr_globals.viewport.zw;
    let ndc = vec2<f32>(
        texel.x / cr_globals.viewport.x * 2.0 - world.w,
        world.w - texel.y / cr_globals.viewport.y * 2.0,
    );
    return vec4<f32>(ndc, 0.0, world.w);
}

// A device point, in the current target's clip space.
fn cr_device_to_target(device: vec2<f32>) -> vec4<f32> {
    let texel = (device - cr_globals.origin.xy) * cr_globals.viewport.zw;
    return vec4<f32>(
        texel.x / cr_globals.viewport.x * 2.0 - 1.0,
        1.0 - texel.y / cr_globals.viewport.y * 2.0,
        0.0,
        1.0,
    );
}

// The device pixel at a fragment's target coordinate. Shapes and clips are evaluated in device
// pixels in every target, so a half-resolution target samples the same geometry at half the rate.
fn cr_device_position(position: vec2<f32>) -> vec2<f32> {
    return position / cr_globals.viewport.zw + cr_globals.origin.xy;
}

// The corner of `b` a unit-square vertex maps to, with one unit of slack on every side for an
// antialiased edge.
fn cr_inflated_corner(vertex: u32, b: cr_Bounds) -> vec2<f32> {
    let origin = cr_bounds_origin(b) - vec2<f32>(1.0);
    let size = cr_bounds_size(b) + vec2<f32>(2.0);
    return origin + cr_unit_corner(vertex) * size;
}

// The same corner as `cr_inflated_corner`, measured from the bounds' origin.
//
// The fragment stage reads its point as this offset, interpolated, plus the flat origin, so a
// primitive far from the origin of its space keeps its precision.
fn cr_inflated_offset(vertex: u32, b: cr_Bounds) -> vec2<f32> {
    return cr_unit_corner(vertex) * (cr_bounds_size(b) + vec2<f32>(2.0)) - vec2<f32>(1.0);
}
