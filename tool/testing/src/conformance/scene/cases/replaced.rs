//! Objects created at the slot of an object the same transaction removed.

use std::sync::Arc;

use iceblink::{
    AnyId, Built, ChunkKey, ClipLinkEntry, ClipNode, ClipRef, ClipShape, Color, CommitError,
    FrameClipShape, Matrix4, OrderEntry, OrderId, OrderNode, Paint, PaintKind, PaintRef,
    ParamsBlock, ParamsId, ROUND, RejectReason, ShaderDesc, ShaderId, ShaderMode, ShaderQuad,
    ShaderReads, ShaderSource, SpaceRef, Transaction, TransformId, TransformNode,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, pixels, props, quad, record_quads};
use crate::conformance::scene::stage::{Stage, covers};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// The kinds of object a chunk draws with through a frame table, and how it names them.
#[derive(Clone, Copy, Debug)]
enum Kind {
    /// A transform named by the chunk's props.
    Transform,
    /// A transform named by the slot in an item.
    ItemSpace,
    /// A clip named by the chunk's props.
    Clip,
    /// A clip named by the slot in an item.
    ItemClip,
    Paint,
    Params,
    Shader,
}

const KINDS: [Kind; 7] = [
    Kind::Transform,
    Kind::ItemSpace,
    Kind::Clip,
    Kind::ItemClip,
    Kind::Paint,
    Kind::Params,
    Kind::Shader,
];

/// The shader and parameter block a shader quad draws with when neither is the object under test.
struct Shading {
    shader: ShaderId,
    params: ParamsId,
}

/// An object created at the slot of one the same transaction removed is a new object, drawn
/// with its own value. A chunk or node that still names the removed object keeps it in use, and
/// one that names the new object keeps only the new object in use.
pub fn replaced_objects(make: MakeCompositor<'_>) {
    for kind in KINDS {
        for written in [false, true] {
            replaced(make, kind, written);
        }
        kept_in_use(make, kind);
        new_props(make, kind);
    }
    named_by_nodes(make);
    created_twice(make);
    placed_order_nodes(make);
}

/// Replaces the object chunk `key(1)` draws with, writing it first when `written`, and checks
/// the frames that draw the replacement and then remove it.
fn replaced(make: MakeCompositor<'_>, kind: Kind, written: bool) {
    let case = format!("{kind:?}, written first: {written}");
    let mut stage = Stage::full(make);
    let shading = shading(&mut stage);
    let mut tx = stage.tx();
    let old = draw_with(&stage, &shading, &mut tx, kind, key(1), 0);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    if written {
        write(&stage, &mut tx, old, 2);
    }
    let new = replace(&mut stage, &shading, &mut tx, old, kind, key(1), 1);
    stage.commit(tx);
    assert_eq!(
        stage.draw(),
        Built::Frame,
        "{case}: the replacement draws a frame"
    );
    damaged(&stage, kind, &case);
    drawn(&stage, kind, 1, &case);
    stage.draw();

    let mut tx = stage.tx();
    tx.remove(new);
    in_use(
        &mut stage,
        tx,
        &format!("{case}: the chunk recorded with the replacement"),
    );

    let mut tx = stage.tx();
    tx.remove_chunk(key(1));
    tx.remove(new);
    stage.commit(tx);
    stage.draw();
}

/// Checks that a chunk the replacing transaction did not record keeps the removed object in use.
fn kept_in_use(make: MakeCompositor<'_>, kind: Kind) {
    let mut stage = Stage::full(make);
    let shading = shading(&mut stage);
    let mut tx = stage.tx();
    let old = draw_with(&stage, &shading, &mut tx, kind, key(1), 0);
    draw_with_id(&stage, &shading, &mut tx, kind, old, key(2));
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Chunk(key(2)));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    replace(&mut stage, &shading, &mut tx, old, kind, key(1), 1);
    in_use(
        &mut stage,
        tx,
        &format!("{kind:?}: a chunk recorded with the removed object"),
    );
}

/// Replaces the object chunk `key(1)` draws with and gives the chunk props that name the
/// replacement, without recording it. A chunk that names the object through its props keeps only
/// the replacement in use, and one whose items name its slot keeps the removed object in use.
fn new_props(make: MakeCompositor<'_>, kind: Kind) {
    if !matches!(
        kind,
        Kind::Transform | Kind::ItemSpace | Kind::Clip | Kind::ItemClip
    ) {
        return;
    }

    let mut stage = Stage::full(make);
    let shading = shading(&mut stage);
    let mut tx = stage.tx();
    let old = draw_with(&stage, &shading, &mut tx, kind, key(1), 0);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    tx.remove(old);
    stage.ids.free(&[old]);
    let mut chunk_props = stage.props();
    let new = match old {
        AnyId::Transform(_) => {
            let new = tx.create_transform(moved(&stage, 1));
            chunk_props.space = new;
            AnyId::Transform(new)
        }
        AnyId::Clip(_) => {
            let new = tx.create_clip(clip(&stage, 1));
            chunk_props.clip = Some(new);
            AnyId::Clip(new)
        }
        _ => unreachable!("only transforms and clips are named by props"),
    };
    assert_eq!(
        slot(new),
        slot(old),
        "{kind:?}: the freed slot is minted again"
    );
    tx.set_props(key(1), chunk_props);

    if matches!(kind, Kind::ItemSpace | Kind::ItemClip) {
        in_use(
            &mut stage,
            tx,
            &format!("{kind:?}: a chunk whose items name the slot of the removed object"),
        );
        return;
    }

    let case = format!("{kind:?}, named by new props");
    match stage.try_commit(tx) {
        Ok(_) => {}
        Err(error) => panic!(
            "{case}: props that name the replacement keep only the replacement in use, but the \
             commit was rejected: {error}"
        ),
    }
    stage.draw();
    damaged(&stage, kind, &case);
    drawn(&stage, kind, 1, &case);

    let mut tx = stage.tx();
    tx.remove(new);
    in_use(&mut stage, tx, &format!("{case}: the chunk"));
}

/// Checks the nodes that name a replaced transform: an untouched node keeps the removed transform
/// in use, and a node created with the replacement keeps the replacement in use.
fn named_by_nodes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let old = tx.create_transform(moved(&stage, 0));
    let old_clip = tx.create_clip(clip_in(old, 0));
    record_quads(&mut tx, key(1), props(old), &[([0.0, 0.0, 8.0, 8.0], RED)]);
    let mut clipped = stage.props();
    clipped.clip = Some(old_clip);
    record_quads(&mut tx, key(2), clipped, &[([0.0, 0.0, 8.0, 8.0], BLUE)]);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Chunk(key(2)));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    let new = replace_transform(&mut stage, &mut tx, old);
    record_quads(&mut tx, key(1), props(new), &[([0.0, 0.0, 8.0, 8.0], RED)]);
    in_use(
        &mut stage,
        tx,
        "a clip node that still names the removed transform",
    );

    // The old clip's removal comes last, after the new clip names the replacement.
    let mut tx = stage.tx();
    let new = replace_transform(&mut stage, &mut tx, old);
    record_quads(&mut tx, key(1), props(new), &[([0.0, 0.0, 8.0, 8.0], RED)]);
    let new_clip = tx.create_clip(clip_in(new, 1));
    clipped.clip = Some(new_clip);
    let inside = [offset(1), 0.0, 4.0, 8.0];
    record_quads(&mut tx, key(2), clipped, &[(inside, BLUE)]);
    tx.remove(old_clip);
    match stage.try_commit(tx) {
        Ok(_) => {}
        Err(error) => panic!(
            "a clip node that names the replacement keeps only the replacement in use, but the \
             commit was rejected: {error}"
        ),
    }
    stage.draw();
    assert!(
        stage.picture.quad(BLUE).clip.is_some(),
        "the chunk is drawn through the clip in the replacement's space"
    );

    let mut tx = stage.tx();
    tx.remove_chunk(key(1));
    tx.remove(new);
    in_use(&mut stage, tx, "the clip node created with the replacement");
}

/// Checks that a node naming the second of two transforms one transaction created at a slot
/// keeps only the second in use.
fn created_twice(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let first = tx.create_transform(moved(&stage, 0));
    let second = replace_transform(&mut stage, &mut tx, first);
    let clip = tx.create_clip(clip_in(second, 1));
    let mut clipped = stage.props();
    clipped.clip = Some(clip);
    let inside = [offset(1), 0.0, 4.0, 8.0];
    record_quads(&mut tx, key(1), clipped, &[(inside, BLUE)]);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    match stage.try_commit(tx) {
        Ok(_) => {}
        Err(error) => panic!(
            "a clip node that names the second transform created at a slot keeps only it in use, \
             but the commit was rejected: {error}"
        ),
    }
    stage.draw();
    assert!(
        stage.picture.quad(BLUE).clip.is_some(),
        "the chunk is drawn through the clip in the second transform's space"
    );
}

/// Checks an order node replaced in its parent's sequence: the entry of the removed node keeps it
/// in use, and an entry of the replacement in its place commits.
fn placed_order_nodes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let parent = tx.create_order(OrderNode::default());
    let old = tx.create_order(OrderNode::default());
    tx.splice(parent, 0, 0, &[OrderEntry::Node(old)]);
    stage.append(&mut tx, OrderEntry::Node(parent));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    replace_order(&mut stage, &mut tx, old);
    in_use(&mut stage, tx, "the entry of the removed order node");

    let mut tx = stage.tx();
    let new = replace_order(&mut stage, &mut tx, old);
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([0.0, 0.0, 8.0, 8.0], RED)],
    );
    tx.splice(new, 0, 0, &[OrderEntry::Chunk(key(1))]);
    tx.splice(parent, 0, 1, &[OrderEntry::Node(new)]);
    match stage.try_commit(tx) {
        Ok(_) => {}
        Err(error) => panic!(
            "an entry of the replacement in place of the removed order node commits, but the \
             commit was rejected: {error}"
        ),
    }
    stage.draw();
    assert_eq!(
        stage.picture.quads_of(RED).len(),
        1,
        "the replacement draws its sequence"
    );
}

/// Removes transform `old` in `tx`, frees its id and creates a transform at its slot.
fn replace_transform(stage: &mut Stage, tx: &mut Transaction, old: TransformId) -> TransformId {
    tx.remove(old);
    stage.ids.free(&[AnyId::Transform(old)]);
    let new = tx.create_transform(moved(stage, 1));
    assert_eq!(
        new.slot(),
        old.slot(),
        "the freed transform slot is minted again"
    );
    new
}

/// Removes order node `old` in `tx`, frees its id and creates an order node at its slot.
fn replace_order(stage: &mut Stage, tx: &mut Transaction, old: OrderId) -> OrderId {
    tx.remove(old);
    stage.ids.free(&[AnyId::Order(old)]);
    let new = tx.create_order(OrderNode::default());
    assert_eq!(
        new.slot(),
        old.slot(),
        "the freed order slot is minted again"
    );
    new
}

/// Checks that committing `tx` is rejected with `InUse`, which `what` keeps in place.
#[track_caller]
fn in_use(stage: &mut Stage, tx: Transaction, what: &str) {
    let result = stage.try_commit(tx);
    assert!(
        matches!(
            result,
            Err(CommitError {
                reason: RejectReason::InUse,
                ..
            })
        ),
        "{what} keeps the object in use, but the commit returned {result:?}"
    );
}

/// Creates the shader and parameter block shader quads draw with, and commits them.
fn shading(stage: &mut Stage) -> Shading {
    let mut tx = stage.tx();
    let shading = Shading {
        shader: tx.create_shader(shader(0)),
        params: tx.create_params(params(0)),
    };
    stage.commit(tx);
    shading
}

/// Removes `old`, frees its id, creates an object of `kind` at its slot with value `shade`, and
/// records chunk `key` with the new object.
fn replace(
    stage: &mut Stage,
    shading: &Shading,
    tx: &mut Transaction,
    old: AnyId,
    kind: Kind,
    key: ChunkKey,
    shade: u8,
) -> AnyId {
    tx.remove(old);
    stage.ids.free(&[old]);
    let new = draw_with(stage, shading, tx, kind, key, shade);
    assert_eq!(
        slot(new),
        slot(old),
        "{kind:?}: the freed slot is minted again"
    );
    new
}

/// Returns the slot of `id`, whose kind is one of `KINDS`.
fn slot(id: AnyId) -> u32 {
    match id {
        AnyId::Transform(id) => id.slot(),
        AnyId::Clip(id) => id.slot(),
        AnyId::Paint(id) => id.slot(),
        AnyId::Params(id) => id.slot(),
        AnyId::Shader(id) => id.slot(),
        _ => unreachable!("only the kinds of `KINDS` are replaced"),
    }
}

/// Creates an object of `kind` with value `shade`, records chunk `key` drawn with it, and
/// returns its id.
fn draw_with(
    stage: &Stage,
    shading: &Shading,
    tx: &mut Transaction,
    kind: Kind,
    key: ChunkKey,
    shade: u8,
) -> AnyId {
    let id = match kind {
        Kind::Transform | Kind::ItemSpace => {
            AnyId::Transform(tx.create_transform(moved(stage, shade)))
        }
        Kind::Clip | Kind::ItemClip => AnyId::Clip(tx.create_clip(clip(stage, shade))),
        Kind::Paint => AnyId::Paint(tx.create_paint(paint(shade))),
        Kind::Params => AnyId::Params(tx.create_params(params(shade))),
        Kind::Shader => AnyId::Shader(tx.create_shader(shader(shade))),
    };
    draw_with_id(stage, shading, tx, kind, id, key);
    id
}

/// Records chunk `key` drawn with object `id`, named as `kind` names it.
fn draw_with_id(
    stage: &Stage,
    shading: &Shading,
    tx: &mut Transaction,
    kind: Kind,
    id: AnyId,
    key: ChunkKey,
) {
    let mut chunk_props = stage.props();
    match (kind, id) {
        (Kind::Transform, AnyId::Transform(space)) => chunk_props = props(space),
        (Kind::Clip, AnyId::Clip(clip)) => chunk_props.clip = Some(clip),
        _ => {}
    }

    let mut chunk = tx.record(key, chunk_props);
    match id {
        AnyId::Transform(space) if matches!(kind, Kind::ItemSpace) => {
            let mut item = quad([0.0, 0.0, 8.0, 8.0], RED);
            item.space = chunk.space_ref(space);
            chunk.quad(item);
        }
        AnyId::Clip(clip) if matches!(kind, Kind::ItemClip) => {
            let mut item = quad([0.0, 0.0, 8.0, 8.0], RED);
            item.clip = chunk.clip_ref(clip);
            chunk.quad(item);
        }
        AnyId::Paint(paint) => {
            let mut item = quad([0.0, 0.0, 8.0, 8.0], RED);
            item.fill = chunk.paint_ref(PaintKind::SOLID, paint);
            chunk.quad(item);
        }
        AnyId::Params(params) => {
            let shader = chunk.shader_slot(shading.shader);
            let params = chunk.params_slot(params);
            chunk.shader_quad(shader_quad(shader, params));
        }
        AnyId::Shader(shader) => {
            let shader = chunk.shader_slot(shader);
            let params = chunk.params_slot(shading.params);
            chunk.shader_quad(shader_quad(shader, params));
        }
        _ => {
            chunk.quad(quad([0.0, 0.0, 8.0, 8.0], RED));
        }
    }
}

/// Writes `id` with value `shade`, when its kind can be written.
fn write(stage: &Stage, tx: &mut Transaction, id: AnyId, shade: u8) {
    match id {
        AnyId::Transform(id) => tx.set_transform_node(id, moved(stage, shade)),
        AnyId::Clip(id) => tx.set_clip(id, clip(stage, shade)),
        AnyId::Paint(id) => tx.set_paint(id, paint(shade)),
        AnyId::Params(id) => tx.set_params(id, params(shade)),
        _ => {}
    }
}

/// Checks that the last frame redrew the pixels that replacing an object of `kind` with value 0 by
/// one with value 1 changes.
#[track_caller]
fn damaged(stage: &Stage, kind: Kind, case: &str) {
    let changed: &[[f32; 4]] = match kind {
        Kind::Transform | Kind::ItemSpace => &[[0.0, 0.0, 8.0, 8.0], [offset(1), 0.0, 8.0, 8.0]],
        Kind::Clip | Kind::ItemClip => &[[clip_width(0), 0.0, 1.0, 8.0]],
        Kind::Paint | Kind::Params | Kind::Shader => &[[0.0, 0.0, 8.0, 8.0]],
    };
    for &rect in changed {
        assert!(
            covers(stage.damage(), pixels(rect)),
            "{case}: the frame redraws {rect:?}, which the replacement changes, but its damage \
             is {:?}",
            stage.damage()
        );
    }
}

/// Checks that the last frame drew chunk `key(1)` with the value `shade` of an object of `kind`.
#[track_caller]
fn drawn(stage: &Stage, kind: Kind, shade: u8, case: &str) {
    let picture = &stage.picture;
    match kind {
        Kind::Transform | Kind::ItemSpace => {
            let drawn = picture.quad(RED);
            let x = offset(shade);
            assert_eq!(
                drawn.bounds,
                [x, 0.0, 8.0, 8.0],
                "{case}: the quad is drawn under the replacement's matrix"
            );
        }
        Kind::Clip | Kind::ItemClip => {
            let drawn = picture.quad(RED);
            let width = drawn.clip.as_ref().and_then(|clip| match clip.links[..] {
                [
                    ClipLinkEntry {
                        shape: FrameClipShape::RoundedRect { rect, .. },
                        ..
                    },
                ] => Some(rect[2]),
                _ => None,
            });
            assert_eq!(
                width,
                Some(clip_width(shade)),
                "{case}: the quad is drawn through the replacement clip"
            );
        }
        Kind::Paint => {
            let drawn = picture.quads_of(painted(shade));
            assert_eq!(
                drawn.len(),
                1,
                "{case}: the quad is filled with the replacement paint"
            );
        }
        Kind::Params | Kind::Shader => {}
    }
}

/// The x offset of the transform with value `shade`.
fn offset(shade: u8) -> f32 {
    f32::from(shade) * 16.0
}

/// A transform under the stage's root space, moved right by `offset(shade)`.
fn moved(stage: &Stage, shade: u8) -> TransformNode {
    TransformNode {
        local: Matrix4::translation(offset(shade), 0.0, 0.0),
        ..identity(Some(stage.space))
    }
}

/// A clip in the stage's root space whose width grows with `shade`.
fn clip(stage: &Stage, shade: u8) -> ClipNode {
    clip_in(stage.space, shade)
}

/// A clip in `space` whose width grows with `shade`.
fn clip_in(space: TransformId, shade: u8) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, clip_width(shade), 128.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

/// The width of the clip with value `shade`.
fn clip_width(shade: u8) -> f32 {
    4.0 + f32::from(shade)
}

/// The colour of the paint with value `shade`.
fn painted(shade: u8) -> [u8; 4] {
    [0, 255, 60 * shade, 255]
}

/// A solid paint of `painted(shade)`.
fn paint(shade: u8) -> Paint {
    let [r, g, b, a] = painted(shade).map(|channel| f32::from(channel) / 255.0);
    Paint::Solid(Color([r, g, b, a]))
}

/// A parameter block whose first user byte is `shade`.
fn params(shade: u8) -> ParamsBlock {
    let mut user = [0; 64];
    user[0] = shade;
    ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user,
    }
}

/// A paint shader whose source differs with `shade`.
fn shader(shade: u8) -> ShaderDesc {
    ShaderDesc {
        mode: ShaderMode::PAINT,
        source: ShaderSource {
            wgsl: Arc::from(format!("// {shade}")),
            prebuilt: None,
        },
        params_size: 0,
        reads: ShaderReads::default(),
    }
}

/// A shader quad of 8 by 8 at the origin with shader slot `shader` and parameter slot `params`.
fn shader_quad(shader: u32, params: u32) -> ShaderQuad {
    ShaderQuad {
        bounds: [0.0, 0.0, 8.0, 8.0],
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        shader,
        params,
        opacity: 1.0,
    }
}
