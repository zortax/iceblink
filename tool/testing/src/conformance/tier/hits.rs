//! Rules of hit testing: order, shapes, clips, transforms and what is never hit.

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Rect, Shape, Stroke};
use iceblink::{
    BlendMode, ChunkFlags, ChunkKey, ClipId, ClipNode, ClipRef, ClipShape, Color, EffectNode,
    Extend, FillRule, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat, LocalClip,
    LocalParent, Matrix4, OrderEntry, OrderNode, Paint, PaintKind, PaintRef, PathClip, PathFill,
    PathItem, PathStroke, ROUND, Shadow, SpaceRef, TransformNode,
};

use crate::conformance::tier::stage::{Factory, Stage, identity, props, quad, quads};

/// Returns the chunks and item indices under `point`, front to back.
fn under(stage: &Stage, point: [f32; 2]) -> Vec<(ChunkKey, u32)> {
    stage
        .compositor
        .hit_test(stage.output, point)
        .iter()
        .map(|hit| (hit.chunk, hit.item))
        .collect()
}

/// An effect that does nothing but clip with `clip`.
fn clipped(clip: ClipId) -> EffectNode {
    EffectNode {
        clip: Some(clip),
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from([]),
        isolate: false,
    }
}

/// Hits come front to back: later chunks of the paint order first, and later items of a chunk
/// before earlier ones.
pub fn hits_come_front_to_back(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        props(stage.space),
        &[[0.0, 0.0, 100.0, 100.0], [50.0, 50.0, 100.0, 100.0]],
    );
    quads(
        &mut tx,
        ChunkKey::new(2, 0),
        props(stage.space),
        &[[40.0, 40.0, 20.0, 20.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(
        under(&stage, [55.0, 55.0]),
        vec![
            (ChunkKey::new(2, 0), 0),
            (ChunkKey::new(1, 0), 1),
            (ChunkKey::new(1, 0), 0)
        ]
    );
    assert_eq!(under(&stage, [180.0, 20.0]), vec![]);
}

/// A quad is hit inside its rounded rectangle exactly, and not in a corner outside its arc.
pub fn rounded_corners_are_hit_exactly(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(stage.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let mut rounded = quad([0.0, 0.0, 100.0, 100.0], fill);
    rounded.radii = [20.0; 8];
    chunk.quad(rounded);
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(under(&stage, [10.0, 10.0]).len(), 1);
    assert_eq!(under(&stage, [5.0, 5.0]).len(), 0);
    assert_eq!(under(&stage, [5.0, 50.0]).len(), 1);
}

/// Clip chains and the clips of the effects above an item cut where it is hit.
pub fn clips_cut_hits(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 100.0, 200.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let effect_clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 200.0, 100.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let effect = tx.create_effect(clipped(effect_clip));
    let group = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let mut clipped = props(stage.space);
    clipped.clip = Some(clip);
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        clipped,
        &[[0.0, 0.0, 200.0, 200.0]],
    );
    tx.splice(group, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    stage.push(&mut tx, OrderEntry::Node(group));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(under(&stage, [50.0, 50.0]).len(), 1);
    assert_eq!(under(&stage, [150.0, 50.0]).len(), 0);
    assert_eq!(under(&stage, [50.0, 150.0]).len(), 0);
}

/// A rounded clip link hits nothing outside its rectangle grown by half a unit, even where the
/// rounded-rectangle distance of a thin elliptical corner gives coverage there.
pub fn soft_clip_links_hit_nothing_outside_their_box(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [5.25, 3.25, 56.0, 104.5],
            radii: [0.25, 3.0, 0.25, 3.0, 0.0, 0.0, 0.0, 0.0],
            shapes: ROUND,
        },
    });
    let effect = tx.create_effect(clipped(clip));
    let group = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        props(stage.space),
        &[[0.0, 0.0, 200.0, 200.0]],
    );
    tx.splice(group, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    stage.push(&mut tx, OrderEntry::Node(group));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(under(&stage, [30.0, 50.0]).len(), 1, "inside the link");
    assert_eq!(
        under(&stage, [61.5, 2.8]).len(),
        1,
        "on the ramp, 0.45 above the top edge"
    );
    assert_eq!(
        under(&stage, [61.5, 2.5]).len(),
        0,
        "past the ramp, 0.75 above the top edge"
    );
}

/// The local point of a hit undoes the item's world matrix and its chunk's offset, and an item
/// whose space collapses the plane is not hit.
pub fn the_local_point_undoes_the_transform_and_the_offset(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let scaled = tx.create_transform(TransformNode {
        local: Matrix4::scale(2.0, 2.0, 1.0).then(&Matrix4::translation(10.0, 10.0, 0.0)),
        ..identity(Some(stage.space))
    });
    let flat = tx.create_transform(TransformNode {
        local: Matrix4::scale(0.0, 1.0, 1.0),
        ..identity(Some(stage.space))
    });
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        props(scaled),
        &[[0.0, 0.0, 40.0, 40.0]],
    );
    quads(
        &mut tx,
        ChunkKey::new(2, 0),
        props(flat),
        &[[0.0, 0.0, 200.0, 200.0]],
    );
    tx.set_offset(ChunkKey::new(1, 0), [5.0, 0.0]);
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    let hits = stage.compositor.hit_test(stage.output, [40.0, 30.0]);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk, ChunkKey::new(1, 0));
    assert_eq!(hits[0].local, [10.0, 10.0]);
}

/// A path is hit inside its fill by its fill rule.
pub fn paths_are_hit_by_their_fill_rule(factory: Factory<'_>) {
    for (rule, hole) in [(FillRule::EVEN_ODD, 0), (FillRule::NON_ZERO, 1)] {
        let mut stage = Stage::new(factory);
        let mut tx = stage.tx();
        let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
        let mut outline = BezPath::new();
        outline.extend(Rect::new(0.0, 0.0, 100.0, 100.0).path_elements(0.1));
        outline.extend(Rect::new(25.0, 25.0, 75.0, 75.0).path_elements(0.1));
        let mut chunk = tx.record(ChunkKey::new(1, 0), props(stage.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.path(PathItem {
            path: Arc::new(outline),
            placement: Affine::IDENTITY,
            brush: Affine::IDENTITY,
            fill: Some(PathFill { paint: fill, rule }),
            stroke: None,
            clips: Arc::from([]),
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
        });
        stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
        stage.commit(tx);
        stage.draw_at(0);
        assert_eq!(under(&stage, [10.0, 10.0]).len(), 1);
        assert_eq!(under(&stage, [50.0, 50.0]).len(), hole);
    }
}

/// Shadows are never hit, and neither are the items of hit-transparent chunks.
pub fn shadows_and_transparent_chunks_are_never_hit(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(stage.space));
    chunk.shadow(Shadow {
        shape_bounds: [0.0, 0.0, 100.0, 100.0],
        radii: [0.0; 8],
        caster_bounds: [0.0, 0.0, 100.0, 100.0],
        caster_radii: [0.0; 8],
        shapes: ROUND,
        color: Color([0.0, 0.0, 0.0, 1.0]),
        blur: 4.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        inset: 0,
    });
    let mut transparent = props(stage.space);
    transparent.flags = ChunkFlags::HIT_TRANSPARENT;
    quads(
        &mut tx,
        ChunkKey::new(2, 0),
        transparent,
        &[[0.0, 0.0, 100.0, 100.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(under(&stage, [50.0, 50.0]), vec![]);
}

/// Soft clip links, chunk-local clips, stroke bands, path clips and rounded image frames are hit
/// exactly.
pub fn clip_links_strokes_and_frames_are_hit_exactly(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let image = tx.create_image(ImageDesc {
        width: 4,
        height: 4,
        format: ImageFormat::RGBA8,
    });

    // A quad through a chunk-local clip with corners of radius 20, which covers up to half a
    // unit outside its arc.
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(stage.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let rounded = chunk.local_clip(LocalClip {
        parent: LocalParent::Chunk,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 100.0, 100.0],
            radii: [20.0; 8],
            shapes: ROUND,
        },
    });
    let mut clipped = quad([0.0, 0.0, 100.0, 100.0], fill);
    clipped.clip = rounded;
    chunk.quad(clipped);

    // A stroked line across y = 150, and a square path filled only inside a path clip.
    let mut line = BezPath::new();
    line.move_to((0.0, 150.0));
    line.line_to((200.0, 150.0));
    chunk.path(PathItem {
        path: Arc::new(line),
        placement: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: None,
        stroke: Some(PathStroke {
            paint: fill,
            style: Stroke::new(10.0),
        }),
        clips: Arc::from([]),
        clip: ClipRef::NONE,
        space: SpaceRef::INHERIT,
    });
    let square = BezPath::from_vec(
        Rect::new(120.0, 0.0, 200.0, 80.0)
            .path_elements(0.1)
            .collect(),
    );
    let corner = BezPath::from_vec(
        Rect::new(120.0, 0.0, 160.0, 40.0)
            .path_elements(0.1)
            .collect(),
    );
    chunk.path(PathItem {
        path: Arc::new(square),
        placement: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint: fill,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from([PathClip {
            path: Arc::new(corner),
            rule: FillRule::NON_ZERO,
        }]),
        clip: ClipRef::NONE,
        space: SpaceRef::INHERIT,
    });

    // An image whose frame is a circle of radius 20 around (40, 180).
    let slot = chunk.image_slot(image);
    chunk.image(Image {
        bounds: [20.0, 160.0, 40.0, 40.0],
        frame: [20.0, 160.0, 40.0, 40.0],
        source: [0.0, 0.0, 4.0, 4.0],
        radii: [20.0; 8],
        shapes: ROUND,
        paint: PaintRef::NONE,
        image: slot,
        opacity: 1.0,
        clip: ClipRef::NONE,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::default(),
        extend: [Extend::NONE; 2],
        filter: ImageFilter::LINEAR,
    });
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.commit(tx);
    stage.draw_at(0);

    let hit = |stage: &Stage, point: [f32; 2]| !under(stage, point).is_empty();
    // The arc's centre is (20, 20), so (6, 6) lies 0.2 inside it and (5.5, 5.5) 0.5 outside.
    assert!(hit(&stage, [6.0, 6.0]));
    assert!(!hit(&stage, [5.5, 5.5]));
    assert!(hit(&stage, [100.0, 154.0]));
    assert!(!hit(&stage, [100.0, 156.0]));
    assert!(hit(&stage, [130.0, 10.0]));
    assert!(!hit(&stage, [180.0, 60.0]));
    assert!(hit(&stage, [40.0, 180.0]));
    assert!(!hit(&stage, [23.0, 163.0]));
}
