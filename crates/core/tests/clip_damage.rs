//! The damage of clip writes, which covers only the pixels whose clip coverage changed.

mod common;

use std::sync::Arc;

use iceblink::{
    BlendMode, Built, ChunkKey, ClipId, ClipNode, ClipShape, DeviceRect, EffectNode, ImageDesc,
    ImageFormat, ImageWrite, LocalClip, LocalParent, MaskMode, Matrix4, OrderEntry, OrderNode,
    PaintRef, ROUND, TransformId,
};
use iceblink::{Compositor, OutputConfig};

use common::{Scene, identity, props, quad, rect};

/// Builds and presents a frame, and returns its draw damage sorted by row.
///
/// Panics if the damage is the whole output.
fn damage_of(scene: &mut Scene) -> Vec<DeviceRect> {
    assert_eq!(scene.build(), Built::Frame);
    scene.done(common::presented());

    let mut rects = scene.damage().expect("the damage is not the whole output");
    rects.sort_by_key(|rect| (rect.origin.y, rect.origin.x));
    rects
}

/// Reports whether `rects` cover every pixel of `expected`.
fn covers(rects: &[DeviceRect], expected: DeviceRect) -> bool {
    let area: i64 = rects
        .iter()
        .filter_map(|held| held.intersection(expected))
        .map(|part| i64::from(part.size.width) * i64::from(part.size.height))
        .sum();
    area == i64::from(expected.size.width) * i64::from(expected.size.height)
}

fn area(rects: &[DeviceRect]) -> i64 {
    rects
        .iter()
        .map(|rect| i64::from(rect.size.width) * i64::from(rect.size.height))
        .sum()
}

/// Returns a clip node under `parent` over `rect` in `space` with `radii`.
fn rounded(
    space: TransformId,
    parent: Option<ClipId>,
    rect: [f32; 4],
    radii: [f32; 8],
) -> ClipNode {
    ClipNode {
        parent,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii,
            shapes: ROUND,
        },
    }
}

/// Records chunk `key` in the scene's root space with one quad over `bounds`, drawn through
/// `clip`, and appends it to the root.
fn clipped_quad(scene: &mut Scene, key: ChunkKey, clip: ClipId, bounds: [f32; 4]) {
    let mut tx = scene.tx();
    let mut chunk_props = props(scene.space);
    chunk_props.clip = Some(clip);
    let mut chunk = tx.record(key, chunk_props);
    chunk.quad(quad(bounds, PaintRef::NONE));
    tx.splice(scene.root, scene.entries, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.entries += 1;
}

/// Creates clip `node` in its own commit and returns its id.
fn create(scene: &mut Scene, node: ClipNode) -> ClipId {
    let mut tx = scene.tx();
    let clip = tx.create_clip(node);
    scene.commit(tx);
    clip
}

#[test]
fn a_clip_edge_move_damages_the_strip_between_the_edges() {
    let mut scene = Scene::new();
    let space = scene.space;
    let clip = create(
        &mut scene,
        rounded(space, None, [0.0, 0.0, 40.0, 100.0], [0.0; 8]),
    );
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 0.0, 100.0, 10.0],
    );
    assert_eq!(
        damage_of(&mut scene),
        vec![rect(0, 0, 40, 10)],
        "ink is cut to the clip"
    );

    let mut tx = scene.tx();
    tx.set_clip(
        clip,
        rounded(space, None, [0.0, 0.0, 80.0, 100.0], [0.0; 8]),
    );
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(40, 0, 40, 10)]);
}

// The chunk is damaged where its quad's coverage changed and where the clip covers differently,
// not over all of its old and new ink.
#[test]
fn a_chunk_recorded_again_under_a_widened_clip_damages_only_the_strips_that_changed() {
    let mut scene = Scene::new();
    let space = scene.space;
    let clip = create(
        &mut scene,
        rounded(space, None, [0.0, 0.0, 60.0, 100.0], [0.0; 8]),
    );
    let key = ChunkKey::new(1, 0);
    clipped_quad(&mut scene, key, clip, [0.0, 0.0, 50.0, 80.0]);
    damage_of(&mut scene);

    let mut tx = scene.tx();
    tx.set_clip(
        clip,
        rounded(space, None, [0.0, 0.0, 70.0, 100.0], [0.0; 8]),
    );
    let mut chunk_props = props(space);
    chunk_props.clip = Some(clip);
    let mut chunk = tx.record(key, chunk_props);
    chunk.quad(quad([0.0, 0.0, 58.0, 80.0], PaintRef::NONE));
    scene.commit(tx);
    let damage = damage_of(&mut scene);
    assert!(
        covers(&damage, rect(50, 0, 8, 80)),
        "the grown strip: {damage:?}"
    );
    assert!(
        area(&damage) <= 10 * 80,
        "only the strip the quad grew by and the clip's moved edge: {damage:?}"
    );
}

#[test]
fn a_clip_edge_moved_beyond_every_item_damages_nothing() {
    let mut scene = Scene::new();
    let output = scene.output;
    scene.compositor.configure_output(
        output,
        OutputConfig {
            size: [1600, 1000],
            scale: 1.0,
            opaque: true,
        },
    );
    let space = scene.space;
    let clip = create(
        &mut scene,
        rounded(space, None, [0.0, 42.0, 1600.0, 645.0], [0.0; 8]),
    );
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 42.0, 275.0, 640.0],
    );
    scene.draw();

    let mut tx = scene.tx();
    tx.set_clip(
        clip,
        rounded(space, None, [0.0, 42.0, 1600.0, 640.0], [0.0; 8]),
    );
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });
}

#[test]
fn a_corner_radius_change_damages_the_corner() {
    let mut scene = Scene::new();
    let space = scene.space;
    let corner = |radius: f32| {
        let mut radii = [0.0; 8];
        radii[0] = radius;
        radii[1] = radius;
        rounded(space, None, [0.0, 0.0, 100.0, 100.0], radii)
    };
    let clip = create(&mut scene, corner(10.0));
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 0.0, 100.0, 100.0],
    );
    scene.draw();

    let mut tx = scene.tx();
    tx.set_clip(clip, corner(20.0));
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(0, 0, 20, 20)]);
}

#[test]
fn a_rounded_clip_that_narrows_damages_its_moved_edge_and_the_corners_beside_it() {
    let mut scene = Scene::new();
    let space = scene.space;
    let narrowed = |width: f32| rounded(space, None, [0.0, 0.0, width, 100.0], [10.0; 8]);
    let clip = create(&mut scene, narrowed(100.0));
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 0.0, 100.0, 100.0],
    );
    scene.draw();

    let mut tx = scene.tx();
    tx.set_clip(clip, narrowed(80.0));
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(79, 0, 21, 100)), "the edge: {rects:?}");
    assert!(
        covers(&rects, rect(70, 0, 30, 10)),
        "the top corner: {rects:?}"
    );
    assert!(
        covers(&rects, rect(70, 90, 30, 10)),
        "the bottom corner: {rects:?}"
    );
    assert!(area(&rects) < 5000, "the rest is kept: {rects:?}");
}

#[test]
fn a_parent_clip_write_damages_what_its_children_clip() {
    let mut scene = Scene::new();
    let space = scene.space;
    let parent = create(
        &mut scene,
        rounded(space, None, [0.0, 0.0, 100.0, 100.0], [0.0; 8]),
    );
    let child = create(
        &mut scene,
        rounded(space, Some(parent), [0.0, 0.0, 50.0, 200.0], [0.0; 8]),
    );
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        child,
        [0.0, 0.0, 200.0, 200.0],
    );
    scene.draw();

    let mut tx = scene.tx();
    tx.set_clip(
        parent,
        rounded(space, None, [0.0, 0.0, 100.0, 60.0], [0.0; 8]),
    );
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(0, 60, 50, 40)]);
}

#[test]
fn a_transform_write_of_a_clip_space_damages_the_strips_the_clip_moved_over() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let moving = tx.create_transform(identity(Some(scene.space)));
    scene.commit(tx);
    let clip = create(
        &mut scene,
        rounded(moving, None, [0.0, 0.0, 40.0, 100.0], [0.0; 8]),
    );
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 0.0, 100.0, 10.0],
    );
    scene.draw();

    let mut tx = scene.tx();
    tx.set_transform(moving, Matrix4::translation(10.0, 0.0, 0.0));
    scene.commit(tx);
    assert_eq!(
        damage_of(&mut scene),
        vec![rect(0, 0, 10, 10), rect(40, 0, 10, 10)]
    );
}

#[test]
fn an_effect_clip_write_damages_the_group_where_the_clip_changed() {
    let mut scene = Scene::new();
    let space = scene.space;
    let clip = create(
        &mut scene,
        rounded(space, None, [0.0, 0.0, 30.0, 100.0], [0.0; 8]),
    );
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: Some(clip),
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
        isolate: false,
    });
    let child = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(space));
    chunk.quad(quad([0.0, 0.0, 100.0, 10.0], PaintRef::NONE));
    tx.splice(child, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(child)]);
    scene.commit(tx);
    scene.draw();
    assert_eq!(
        scene.frame.groups[0].bounds,
        [0.0, 0.0, 30.0, 10.0],
        "the group is cut to its clip"
    );

    let mut tx = scene.tx();
    tx.set_clip(
        clip,
        rounded(space, None, [0.0, 0.0, 60.0, 100.0], [0.0; 8]),
    );
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(30, 0, 30, 10)]);
}

#[test]
fn a_clip_write_damages_items_of_local_clips_under_it_where_it_changed() {
    let mut scene = Scene::new();
    let space = scene.space;
    let outer = create(
        &mut scene,
        rounded(space, None, [0.0, 0.0, 40.0, 100.0], [0.0; 8]),
    );
    let mut tx = scene.tx();
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(space));
    let local = chunk.local_clip(LocalClip {
        parent: LocalParent::Node(outer),
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 200.0, 200.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    chunk.quad(iceblink::Quad {
        clip: local,
        ..quad([0.0, 0.0, 100.0, 10.0], PaintRef::NONE)
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_clip(
        outer,
        rounded(space, None, [0.0, 0.0, 80.0, 100.0], [0.0; 8]),
    );
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(40, 0, 40, 10)]);
}

#[test]
fn a_mask_clip_written_again_unchanged_damages_nothing() {
    let mut scene = Scene::new();
    let space = scene.space;
    let mut tx = scene.tx();
    let image = tx.create_image(ImageDesc {
        width: 4,
        height: 4,
        format: ImageFormat::A8,
    });
    tx.write_image(
        image,
        ImageWrite {
            rect: rect(0, 0, 4, 4),
            bytes: Arc::from(vec![255_u8; 16]),
            offset: 0,
            stride: 4,
        },
    );
    scene.commit(tx);
    let mask = ClipNode {
        parent: None,
        space,
        shape: ClipShape::Mask {
            image,
            rect: [0.0, 0.0, 40.0, 40.0],
            source: [0.0, 0.0, 4.0, 4.0],
            mode: MaskMode::ALPHA,
        },
    };
    let clip = create(&mut scene, mask.clone());
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 0.0, 100.0, 100.0],
    );
    scene.draw();

    let mut tx = scene.tx();
    tx.set_clip(clip, mask);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });
}

#[test]
fn an_item_under_a_rounded_clip_in_an_enlarged_space_has_the_clip_ramp_in_its_ink() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let enlarged = tx.create_transform(iceblink::TransformNode {
        local: Matrix4::scale(2.0, 2.0, 1.0),
        ..identity(Some(scene.space))
    });
    scene.commit(tx);
    let clip = create(
        &mut scene,
        rounded(enlarged, None, [10.0, 10.0, 20.0, 20.0], [2.0; 8]),
    );
    clipped_quad(
        &mut scene,
        ChunkKey::new(1, 0),
        clip,
        [0.0, 0.0, 100.0, 100.0],
    );
    assert_eq!(damage_of(&mut scene), vec![rect(19, 19, 42, 42)]);
}
