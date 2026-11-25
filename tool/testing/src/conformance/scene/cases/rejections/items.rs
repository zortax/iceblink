//! Items, nodes and command values that are not well formed.

use std::sync::Arc;

use iceblink::kurbo::BezPath;
use iceblink::{
    BorderStyle, ClipNode, ClipRef, ClipShape, Color, EffectNode, FillRule, Filter, MarkFlags,
    MarkPayload, MarkPlacement, Marks, Matrix4, Paint, PaintRef, PathFill, PathItem, ROUND,
    RejectReason, ScrollBehaviour, ScrollNode, ScrollPhysics, SnapKind, Sources, SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{faded, identity, key, quad, record_quads};
use crate::conformance::scene::cases::rejections::{RED, rejects};
use crate::conformance::scene::stage::Stage;

/// Items, nodes and command values are well formed.
pub fn invalid_items(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([0.0, f32::NAN, 8.0, 8.0], RED)],
    );
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a quad with a NaN bound",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    let mut styled = quad([0.0, 0.0, 8.0, 8.0], RED);
    styled.style = BorderStyle(9);
    chunk.quad(styled);
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "an unlisted border style",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.empty_chunk(key(1), stage.props());
    chunk.quads.push(quad([0.0, 0.0, 8.0, 8.0], RED));
    tx.record_chunk(chunk);
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a chunk whose ops skip an item",
    );

    let mut tx = stage.tx();
    let mut node = identity(None);
    node.local = Matrix4::translation(f32::INFINITY, 0.0, 0.0);
    tx.create_transform(node);
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a transform with an infinite matrix",
    );

    stage.add_quads(key(2), &[([20.0, 20.0, 8.0, 8.0], [0, 0, 255, 255])]);
    let mut tx = stage.tx();
    tx.set_offset(key(2), [f32::NAN, 0.0]);
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a NaN chunk offset",
    );

    let mut tx = stage.tx();
    tx.create_effect(EffectNode {
        opacity: f32::NAN,
        ..faded(1.0)
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "an effect with a NaN opacity",
    );

    let mut tx = stage.tx();
    tx.create_scroll(ScrollNode {
        space: stage.space,
        port: [0.0, 0.0, 10.0, 10.0],
        content: [10.0, 20.0],
        offset: [0.0; 2],
        behaviour: Some(ScrollBehaviour {
            physics: ScrollPhysics::Instant,
            snap: Arc::from([]),
            snap_kind: SnapKind::Mandatory,
            bounce: Sources(1 << 7),
            user_scrollable: [false, true],
            chain: Sources::NONE,
        }),
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a scroll behaviour bouncing an unlisted source",
    );

    let mut tx = stage.tx();
    tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, f32::INFINITY, 8.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a clip with an infinite rectangle",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(3), stage.props());
    chunk.path(nan_path());
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a path with a NaN point",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(3), stage.props());
    let payload = chunk.payload(Arc::new(MarkPayload {
        shapes: vec![Arc::new(BezPath::new())],
        placements: vec![placement(0.0, 0), placement(4.0, 7)],
        ..MarkPayload::default()
    }));
    chunk.marks(Marks {
        bounds: [0.0, 0.0, 16.0, 16.0],
        paint: PaintRef::color(RED),
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements: 1,
        first: 1,
        payload,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::default(),
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a placement drawn past its first that names no shape",
    );

    let mut tx = stage.tx();
    tx.create_effect(EffectNode {
        filters: Arc::from([Filter::Blur(f32::NAN)]),
        ..faded(1.0)
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a blur of NaN deviation",
    );

    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([0.5, 0.0, 0.0, 0.5])));
    stage.commit(tx);
    let mut tx = stage.tx();
    tx.set_paint(paint, Paint::Solid(Color([0.8, 0.0, 0.0, 0.5])));
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a solid paint with red above its alpha",
    );

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(4),
        stage.props(),
        &[([0.0, 0.0, 4.0, 4.0], [255, 0, 0, 128])],
    );
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "an inline colour with red above its alpha",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(4), stage.props());
    chunk.quad(quad([0.0, 0.0, 4.0, 4.0], RED));
    chunk.hint([0.0, 0.0, f32::INFINITY, 4.0]);
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidItem,
        "a chunk with an infinite hint",
    );
}

/// A filled triangle path with a NaN point.
fn nan_path() -> PathItem {
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((f64::NAN, 4.0));
    path.line_to((4.0, 4.0));
    path.close_path();
    PathItem {
        path: Arc::new(path),
        placement: Default::default(),
        brush: Default::default(),
        fill: Some(PathFill {
            paint: PaintRef::color(RED),
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    }
}

/// A mark placement at `(x, 0)` of shape `shape`.
fn placement(x: f32, shape: u32) -> MarkPlacement {
    MarkPlacement {
        x,
        y: 0.0,
        shape,
        _pad: 0,
    }
}
