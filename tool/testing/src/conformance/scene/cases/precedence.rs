//! Which rule a rejection reports when a transaction breaks several.

use std::sync::Arc;

use iceblink::{
    ChunkKey, ClientId, ClipNode, ClipShape, Color, CommitError, Filter, LocalClip, LocalParent,
    Matrix4, OrderEntry, OrderNode, Paint, PaintId, PaintKind, PaintRef, ParamsBlock, ROUND,
    RejectReason, ScrollNode, ShaderDesc, ShaderMode, ShaderReads, ShaderSource, Transaction,
    TransformId,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props, quad, record_quads};
use crate::conformance::scene::stage::Stage;

const RED: [u8; 4] = [255, 0, 0, 255];

/// A rejection is reported at the first command, in transaction order, that breaks a rule, with
/// the reason of the first rule in the spec's order.
pub fn rejection_precedence(make: MakeCompositor<'_>) {
    let mut failures = Vec::new();
    let mut expect = |stage: &mut Stage, tx: Transaction, want: (RejectReason, u32), case: &str| {
        let found = stage
            .try_commit(tx)
            .map(|_| ())
            .map_err(|CommitError { reason, command }| (reason, command));
        let want = Err((want.0, Some(want.1)));
        if found != want {
            failures.push(format!("{case}: expected {want:?}, got {found:?}"));
        }
    };

    let mut stage = Stage::full(make);
    let named = named_chunk(&mut stage);
    let stale = stale_transform(&mut stage);
    let [first, second] = two_transforms(&mut stage);

    let mut tx = stage.tx();
    record_quads(&mut tx, key(2), stage.props(), &[(nan_bounds(), RED)]);
    tx.set_offset(key(77), [1.0, 0.0]);
    expect(
        &mut stage,
        tx,
        (RejectReason::UnknownChunk, 1),
        "a command rule beats the items of an earlier chunk",
    );

    let mut tx = stage.tx();
    tx.set_props(key(77), props(stale));
    expect(
        &mut stage,
        tx,
        (RejectReason::StaleId, 0),
        "a stale id comes before an unknown chunk",
    );

    let mut tx = stage.tx();
    tx.set_offset(key(77), [f32::NAN, 0.0]);
    expect(
        &mut stage,
        tx,
        (RejectReason::UnknownChunk, 0),
        "an unknown chunk comes before a NaN offset",
    );

    let mut tx = stage.tx();
    tx.set_transform(stale, Matrix4::translation(f32::NAN, 0.0, 0.0));
    expect(
        &mut stage,
        tx,
        (RejectReason::StaleId, 0),
        "a stale id comes before a NaN matrix",
    );

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(3),
        stage.props(),
        &[([0.0, 0.0, 4.0, 4.0], RED)],
    );
    record_quads(&mut tx, key(4), stage.props(), &[(nan_bounds(), RED)]);
    record_quads(&mut tx, key(3), stage.props(), &[(nan_bounds(), RED)]);
    expect(
        &mut stage,
        tx,
        (RejectReason::InvalidItem, 1),
        "the earliest of two bad chunks",
    );

    let mut tx = stage.tx();
    tx.remove(named.paint);
    record_quads(&mut tx, key(5), stage.props(), &[(nan_bounds(), RED)]);
    expect(
        &mut stage,
        tx,
        (RejectReason::InUse, 0),
        "a removal still in use before a later bad chunk",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(6), stage.props());
    let gradient = chunk.paint_ref(PaintKind::GRADIENT, named.paint);
    chunk.quad(quad(nan_bounds(), RED));
    let mut wrong = quad([0.0, 0.0, 4.0, 4.0], RED);
    wrong.fill = gradient;
    chunk.quad(wrong);
    expect(
        &mut stage,
        tx,
        (RejectReason::PaintKindChanged, 0),
        "a paint of the wrong kind comes before a NaN item in one chunk",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(named.key, stage.props());
    let fill = chunk.paint_ref(PaintKind::SOLID, named.paint);
    let mut item = quad([0.0, 0.0, 4.0, 4.0], RED);
    item.fill = fill;
    chunk.quad(item);
    tx.remove(named.paint);
    expect(
        &mut stage,
        tx,
        (RejectReason::StaleId, 0),
        "a chunk taking a removed paint through its refs",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(named.key, stage.props());
    let mut item = quad([0.0, 0.0, 4.0, 4.0], RED);
    item.fill = PaintRef {
        kind: PaintKind::SOLID,
        _pad: [0; 3],
        index: named.paint.slot(),
    };
    chunk.quad(item);
    tx.remove(named.paint);
    expect(
        &mut stage,
        tx,
        (RejectReason::InvalidItem, 0),
        "a chunk naming a removed paint by its raw slot",
    );

    let mut tx = stage.tx();
    tx.set_transform_node(second, identity(Some(first)));
    tx.set_transform_node(first, identity(Some(second)));
    expect(
        &mut stage,
        tx,
        (RejectReason::Cycle, 0),
        "a cycle at the earliest write of a node on it",
    );

    let mut tx = stage.tx();
    let placed = tx.create_order(OrderNode::default());
    stage.append(&mut tx, OrderEntry::Node(placed));
    stage.commit(tx);
    let mut tx = stage.tx();
    tx.set_root(stage.output, Some(placed));
    tx.splice(placed, 0, 0, &[OrderEntry::Chunk(named.key)]);
    expect(
        &mut stage,
        tx,
        (RejectReason::OrderConflict, 0),
        "a root placed twice before a chunk key placed twice",
    );

    let mut tx = stage.tx();
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 16.0, 16.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    stage.commit(tx);
    let mut tx = stage.tx();
    let mut chunk = tx.record(key(8), stage.props());
    let local = chunk.local_clip(LocalClip {
        parent: LocalParent::Node(clip),
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 8.0, 8.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let mut item = quad([0.0, 0.0, 4.0, 4.0], RED);
    item.clip = local;
    chunk.quad(item);
    tx.remove(clip);
    expect(
        &mut stage,
        tx,
        (RejectReason::StaleId, 0),
        "a chunk whose local clip hangs on a clip node removed later",
    );

    let mut tx = stage.tx();
    let paint_shader = tx.create_shader(shader(ShaderMode::PAINT));
    let params = tx.create_params(ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    let scroll = tx.create_scroll(ScrollNode {
        space: stage.space,
        port: [0.0, 0.0, 10.0, 10.0],
        content: [10.0, 100.0],
        offset: [0.0; 2],
        behaviour: None,
    });
    let gone_shader = tx.create_shader(shader(ShaderMode::FILTER));
    stage.commit(tx);
    let mut tx = stage.tx();
    tx.remove(gone_shader);
    tx.remove(scroll);
    stage.commit(tx);

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(9), stage.props());
    chunk.local_clip(LocalClip {
        parent: LocalParent::Local(3),
        shape: window(),
    });
    tx.set_offset(key(77), [1.0, 0.0]);
    expect(
        &mut stage,
        tx,
        (RejectReason::UnknownChunk, 1),
        "a local clip whose parent is not earlier is checked with the finished chunk",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(9), stage.props());
    chunk.filters(Arc::from([Filter::Shader {
        shader: paint_shader,
        params,
        reach: 0.0,
    }]));
    tx.set_offset(key(77), [1.0, 0.0]);
    expect(
        &mut stage,
        tx,
        (RejectReason::UnknownChunk, 1),
        "a filter chain's shader mode is checked with the finished chunk",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(9), stage.props());
    chunk.filters(Arc::from([Filter::Shader {
        shader: gone_shader,
        params,
        reach: 0.0,
    }]));
    tx.set_offset(key(77), [1.0, 0.0]);
    expect(
        &mut stage,
        tx,
        (RejectReason::StaleId, 0),
        "a filter chain's ids are checked as the recording applies",
    );

    let mut tx = stage.tx();
    tx.acknowledge_scroll(scroll, 1, [f32::NAN, 0.0]);
    expect(
        &mut stage,
        tx,
        (RejectReason::StaleId, 0),
        "a stale scroll id comes before a NaN acknowledged offset",
    );

    let mut foreign = stage.compositor.transaction(ClientId(9_999));
    foreign.create_order(OrderNode::default());
    let found = stage
        .try_commit(foreign)
        .map(|_| ())
        .map_err(|CommitError { reason, command }| (reason, command));
    if found != Err((RejectReason::StaleId, None)) {
        failures.push(format!(
            "a client that is not registered: expected (StaleId, None), got {found:?}"
        ));
    }

    assert!(
        failures.is_empty(),
        "rejection precedence:\n  {}",
        failures.join("\n  ")
    );
}

/// A committed chunk in the root's sequence whose quad takes a solid paint slot.
struct Named {
    key: ChunkKey,
    paint: PaintId,
}

fn named_chunk(stage: &mut Stage) -> Named {
    let key = key(1);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    let mut chunk = tx.record(key, stage.props());
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let mut item = quad([0.0, 0.0, 8.0, 8.0], RED);
    item.fill = fill;
    chunk.quad(item);
    stage.append(&mut tx, OrderEntry::Chunk(key));
    stage.commit(tx);
    Named { key, paint }
}

/// Returns the id of a transform that was created and then removed.
fn stale_transform(stage: &mut Stage) -> TransformId {
    let mut tx = stage.tx();
    let space = tx.create_transform(identity(None));
    stage.commit(tx);
    let mut tx = stage.tx();
    tx.remove(space);
    stage.commit(tx);
    space
}

fn two_transforms(stage: &mut Stage) -> [TransformId; 2] {
    let mut tx = stage.tx();
    let first = tx.create_transform(identity(None));
    let second = tx.create_transform(identity(None));
    stage.commit(tx);
    [first, second]
}

fn nan_bounds() -> [f32; 4] {
    [0.0, f32::NAN, 4.0, 4.0]
}

fn shader(mode: ShaderMode) -> ShaderDesc {
    ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from(""),
            prebuilt: None,
        },
        params_size: 0,
        reads: ShaderReads::default(),
    }
}

fn window() -> ClipShape {
    ClipShape::RoundedRect {
        rect: [0.0, 0.0, 8.0, 8.0],
        radii: [0.0; 8],
        shapes: ROUND,
    }
}
