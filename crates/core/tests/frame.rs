//! Frame assembly: buffer ages, table deltas, opaque covers, groups, paths, backdrops and shader
//! work.

mod common;

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath};
use iceblink::{
    AnyId, Backdrop, BatchKind, BlendMode, BufferAge, Built, ChunkKey, ClipRef, EffectNode,
    FillRule, Filter, OrderEntry, OrderNode, PaintRef, ParamsBlock, ParamsId, PathFill, PathItem,
    ShaderDesc, ShaderId, ShaderMode, ShaderOp, ShaderQuad, ShaderReads, ShaderSource, SpaceRef,
    Transaction, read_extent,
};
use iceblink::{Compositor, CompositorBudgets};

use common::{Scene, props, quad, rect};

#[test]
fn an_opaque_quad_partly_above_the_output_covers_only_pixels_on_the_output() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, -20.0, 100.0, 60.0]]);
    assert_eq!(scene.build(), Built::Frame);

    let covers: Vec<_> = scene
        .frame
        .opaque_covers
        .iter()
        .map(|cover| cover.rect)
        .collect();
    assert_eq!(covers, vec![rect(11, 0, 98, 39)]);
    if let Err(violations) = iceblink_testing::validate_frame(&scene.frame) {
        panic!("{}", iceblink_testing::validate::describe(&violations));
    }
}

fn params() -> ParamsBlock {
    ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    }
}

fn shader(mode: ShaderMode, reads: ShaderReads) -> ShaderDesc {
    ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from("fn shade() {}"),
            prebuilt: None,
        },
        params_size: 0,
        reads,
    }
}

#[test]
fn an_older_image_redraws_what_the_frames_since_presented() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 10.0, 10.0]]);
    scene.draw();
    scene.add_chunk(ChunkKey::new(2, 0), &[[50.0, 50.0, 10.0, 10.0]]);
    assert_eq!(scene.build_aged(BufferAge::Known(2)), Built::Frame);
    scene.done(common::presented());
    let draw = scene.frame.damage.draw;
    assert!(
        draw.intersects(rect(10, 10, 10, 10)),
        "the previous frame's present is owed"
    );
    assert!(draw.intersects(rect(50, 50, 10, 10)));
    assert_eq!(scene.frame.damage.present.rects(), &[rect(50, 50, 10, 10)]);

    scene.add_chunk(ChunkKey::new(3, 0), &[[90.0, 90.0, 10.0, 10.0]]);
    assert_eq!(scene.build_aged(BufferAge::Unknown), Built::Frame);
    assert!(scene.frame.damage.draw.is_full());
}

#[test]
fn tables_are_sent_whole_once_and_then_as_changes() {
    let mut scene = Scene::new();
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 10.0, 10.0]]);
    assert_eq!(scene.build(), Built::Frame);
    assert!(!scene.frame.tables.spaces.replace);
    assert!(
        !scene.frame.tables.paints.changed.is_empty(),
        "the new paint is sent"
    );
    scene.done(common::presented());

    scene.add_chunk(ChunkKey::new(2, 0), &[[30.0, 10.0, 10.0, 10.0]]);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.frame.tables.paints.changed.len(), 1);
    assert!(scene.frame.tables.spaces.is_unchanged());
}

#[test]
fn a_group_partly_above_the_output_is_bounded_by_the_output() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![Filter::Blur(2.0)]),
        isolate: true,
    });
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([10.0, -20.0, 100.0, 60.0], PaintRef::NONE));
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(node)]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);

    // A blur of 2 reaches 8 pixels past the quad.
    assert_eq!(scene.frame.groups[0].bounds, [2.0, 0.0, 116.0, 48.0]);
}

#[test]
fn an_isolating_effect_draws_its_subtree_between_group_markers() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: BlendMode::MULTIPLY,
        filters: Arc::from(Vec::new()),
        isolate: false,
    });
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([10.0, 10.0, 10.0, 10.0], PaintRef::NONE));
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(node)]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);

    let kinds: Vec<BatchKind> = scene.frame.batches.iter().map(|batch| batch.kind).collect();
    assert_eq!(
        kinds,
        vec![
            BatchKind::GroupBegin(0),
            BatchKind::Quads,
            BatchKind::GroupEnd(0)
        ]
    );
    let group = &scene.frame.groups[0];
    assert_eq!(group.bounds, [10.0, 10.0, 10.0, 10.0]);
    assert_eq!((group.opacity, group.blend), (0.5, BlendMode::MULTIPLY));
}

#[test]
fn damage_that_reaches_a_blurred_group_grows_to_what_it_reads() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![Filter::Blur(2.0)]),
        isolate: false,
    });
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([50.0, 50.0, 40.0, 40.0], PaintRef::NONE));
    chunk.quad(quad([60.0, 60.0, 4.0, 4.0], PaintRef::NONE));
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(node)]);
    scene.commit(tx);
    scene.draw();

    // Changing the small quad damages a corner of the group, and the closure takes in all the
    // group reads.
    let mut tx = scene.tx();
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([50.0, 50.0, 40.0, 40.0], PaintRef::NONE));
    chunk.quad(quad([61.0, 60.0, 4.0, 4.0], PaintRef::NONE));
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    let reach = read_extent([50.0, 50.0, 40.0, 40.0], &[Filter::Blur(2.0)]);
    let extent = rect(
        reach[0] as i32,
        reach[1] as i32,
        reach[2] as i32,
        reach[3] as i32,
    );
    let held = scene.damage().expect("the damage is not full");
    assert!(held.iter().any(|rect| rect.contains_rect(extent)));
}

#[test]
fn a_path_gets_one_vector_pass() {
    let mut scene = Scene::new();
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((20.0, 0.0));
    path.line_to((0.0, 20.0));
    path.close_path();
    let mut tx = scene.tx();
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.path(PathItem {
        path: Arc::new(path),
        placement: Affine::translate((40.0, 40.0)),
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint: PaintRef::NONE,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);

    assert_eq!(scene.frame.paths.len(), 1);
    assert_eq!(scene.frame.paths[0].ink, rect(40, 40, 20, 20));
    assert_eq!(scene.frame.vector_plan.passes.len(), 1);
    assert_eq!(
        scene.frame.vector_plan.passes[0].region,
        rect(32, 32, 32, 32)
    );
    assert_eq!(scene.frame.batches[0].kind, BatchKind::Paths { pass: 0 });
    assert_eq!(scene.compositor.counters().frame.vector_passes, 1);
}

#[test]
fn a_backdrop_reads_its_filters_extent() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let chain = chunk.filters(Arc::from(vec![Filter::Blur(1.0)]));
    chunk.backdrop(Backdrop {
        bounds: [20.0, 20.0, 30.0, 30.0],
        filters: chain,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    let backdrop = &scene.frame.backdrops[0];
    assert_eq!(backdrop.bounds, [20.0, 20.0, 30.0, 30.0]);
    assert_eq!(
        backdrop.source,
        read_extent([20.0, 20.0, 30.0, 30.0], &[Filter::Blur(1.0)])
    );
}

#[test]
fn a_clock_reading_shader_is_drawn_on_every_frame() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let timed = tx.create_shader(shader(ShaderMode::PAINT, ShaderReads::TIME));
    let block = tx.create_params(params());
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let shader_slot = chunk.shader_slot(timed);
    let params_slot = chunk.params_slot(block);
    chunk.shader_quad(ShaderQuad {
        bounds: [10.0, 10.0, 10.0, 10.0],
        radii: [0.0; 8],
        shapes: iceblink::ROUND,
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        shader: shader_slot,
        params: params_slot,
        opacity: 1.0,
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert!(matches!(
        scene.frame.resources.shaders.as_slice(),
        [ShaderOp::Create { .. }]
    ));
    assert!(matches!(
        scene.frame.batches[0].kind,
        BatchKind::ShaderQuads { .. }
    ));

    assert_eq!(scene.draw(), Built::Frame, "the shader is drawn again");
    assert!(
        scene.frame.resources.shaders.is_empty(),
        "the shader is created once"
    );
    assert_eq!(scene.damage(), Some(vec![rect(10, 10, 10, 10)]));

    let mut tx = scene.tx();
    tx.remove_chunk(ChunkKey::new(1, 0));
    tx.remove(timed);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert!(matches!(
        scene.frame.resources.shaders.as_slice(),
        [ShaderOp::Destroy { .. }]
    ));
}

#[test]
fn a_shader_created_at_the_slot_of_a_removed_one_replaces_it_on_the_renderer() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let old = tx.create_shader(shader(ShaderMode::PAINT, ShaderReads::default()));
    let block = tx.create_params(params());
    record_shader_quad(&mut tx, &scene, old, block);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);

    let mut tx = scene.tx();
    tx.remove(old);
    scene.ids.free(&[AnyId::Shader(old)]);
    let new = tx.create_shader(shader(ShaderMode::COVERAGE, ShaderReads::default()));
    assert_eq!(new.slot(), old.slot());
    record_shader_quad(&mut tx, &scene, new, block);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert!(
        matches!(
            scene.frame.resources.shaders.as_slice(),
            [ShaderOp::Destroy { .. }, ShaderOp::Create { desc, .. }]
                if desc.mode == ShaderMode::COVERAGE
        ),
        "the frame destroys the removed shader and creates its replacement: {:?}",
        scene.frame.resources.shaders
    );
}

/// Records chunk `(1, 0)` with one shader quad drawn with `shader` and `block`.
fn record_shader_quad(tx: &mut Transaction, scene: &Scene, shader: ShaderId, block: ParamsId) {
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let shader_slot = chunk.shader_slot(shader);
    let params_slot = chunk.params_slot(block);
    chunk.shader_quad(ShaderQuad {
        bounds: [10.0, 10.0, 10.0, 10.0],
        radii: [0.0; 8],
        shapes: iceblink::ROUND,
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        shader: shader_slot,
        params: params_slot,
        opacity: 1.0,
    });
}

#[test]
fn a_parameter_write_damages_the_shader_quads_naming_it() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let plain = tx.create_shader(shader(ShaderMode::COVERAGE, ShaderReads::default()));
    let block = tx.create_params(params());
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let shader_slot = chunk.shader_slot(plain);
    let params_slot = chunk.params_slot(block);
    chunk.shader_quad(ShaderQuad {
        bounds: [10.0, 10.0, 10.0, 10.0],
        radii: [0.0; 8],
        shapes: iceblink::ROUND,
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        shader: shader_slot,
        params: params_slot,
        opacity: 1.0,
    });
    chunk.quad(quad([100.0, 100.0, 10.0, 10.0], PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    let mut moved = params();
    moved.hovered = 1.0;
    tx.set_params(block, moved);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(10, 10, 10, 10)]));
    assert_eq!(scene.frame.tables.params.changed.len(), 1);
}

#[test]
fn a_coverage_shader_becomes_a_quad_without_custom_shaders() {
    let mut caps = common::FULL;
    caps.custom_shaders = false;
    let mut scene = Scene::with_caps(caps);
    let mut tx = scene.tx();
    let plain = tx.create_shader(shader(ShaderMode::COVERAGE, ShaderReads::default()));
    let painted = tx.create_shader(shader(ShaderMode::PAINT, ShaderReads::default()));
    let block = tx.create_params(params());
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let params_slot = chunk.params_slot(block);
    for (index, shader) in [plain, painted].into_iter().enumerate() {
        let shader_slot = chunk.shader_slot(shader);
        chunk.shader_quad(ShaderQuad {
            bounds: [10.0 + 20.0 * index as f32, 10.0, 10.0, 10.0],
            radii: [0.0; 8],
            shapes: iceblink::ROUND,
            border: [0.0; 4],
            fill: PaintRef::NONE,
            stroke: PaintRef::NONE,
            paint_origin: [0.0; 2],
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            shader: shader_slot,
            params: params_slot,
            opacity: 1.0,
        });
    }
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.frame.lanes.quads.drawn(), 1);
    assert!(scene.frame.lanes.shader_quads.sources.is_empty());
}

#[test]
fn budgets_are_kept_and_trimming_an_empty_atlas_frees_nothing() {
    let mut scene = Scene::new();
    assert_eq!(scene.compositor.budgets(), CompositorBudgets::default());
    let budgets = CompositorBudgets {
        atlas_bytes: Some(1 << 20),
    };
    scene.compositor.set_budgets(budgets);
    assert_eq!(scene.compositor.budgets(), budgets);
    assert_eq!(scene.compositor.trim(0), 0);
}

#[test]
fn a_marks_shape_too_large_for_a_cell_is_drawn_as_a_path_beside_its_prims() {
    use iceblink::kurbo::Shape as _;
    use iceblink::{MarkFlags, MarkPayload, MarkPlacement, Marks};

    let mut scene = Scene::new();
    let large = iceblink::kurbo::Rect::new(0.0, 0.0, 300.0, 300.0).to_path(0.1);
    let payload = Arc::new(MarkPayload {
        discs: vec![[20.0, 20.0, 6.0, 0.0]],
        shapes: vec![Arc::new(large)],
        placements: vec![MarkPlacement {
            x: 40.0,
            y: 40.0,
            shape: 0,
            _pad: 0,
        }],
        ..MarkPayload::default()
    });
    let mut tx = scene.tx();
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(scene.space));
    let index = chunk.payload(payload);
    chunk.marks(Marks {
        bounds: [0.0, 0.0, 100.0, 100.0],
        paint: PaintRef::NONE,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 1,
        boxes: 0,
        vertices: 0,
        placements: 1,
        first: 0,
        payload: index,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::default(),
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);

    assert_eq!(scene.frame.lanes.marks.drawn(), 1, "the disc draws");
    assert_eq!(scene.frame.paths.len(), 1);
    assert_eq!(scene.frame.paths[0].ink, rect(40, 40, 60, 60));
}

#[test]
fn the_shader_clock_counts_from_the_first_frame_request() {
    // Eleven days on a clock that counts from boot, where `f32` seconds step by 1/16.
    const BOOT: u64 = 950_400_000_000_000;
    let mut compositor = common::checked();
    let (client, _) = compositor.register_client();
    let output = compositor.create_output(iceblink::OutputConfig {
        size: [64, 64],
        scale: 1.0,
        opaque: true,
    });
    let mut frame = iceblink::Frame::default();
    let mut clocked = |compositor: &mut iceblink_core::CompositorImpl, now_ns, present_ns| {
        let request = iceblink::FrameRequest {
            caps: &common::FULL,
            age: BufferAge::Kept,
            clock: iceblink::FrameClock {
                now_ns,
                present_ns,
                interval_ns: 16_666_667,
            },
            subpixel: None,
        };
        let built = compositor.build_frame(output, &request, &mut frame);
        assert_eq!(built, Built::Frame);
        compositor.frame_done(output, frame.id, &common::presented());
        frame.shader_clock
    };

    let first = clocked(&mut compositor, BOOT, BOOT + 20_000_000);
    let mut tx = compositor.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    compositor.commit(tx).expect("the root commits");
    let second = clocked(&mut compositor, BOOT + 1_000_000_000, BOOT + 1_017_000_000);

    assert!((first.time - 0.020).abs() < 1e-6, "{first:?}");
    assert_eq!(first.delta, 0.0);
    assert!((second.time - 1.017).abs() < 1e-6, "{second:?}");
    assert!((second.delta - 0.997).abs() < 1e-6, "{second:?}");
}

#[test]
fn combined_clip_chains_are_released_once_their_effect_and_chunks_are_gone() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let shape = |rect| iceblink::ClipShape::RoundedRect {
        rect,
        radii: [0.0; 8],
        shapes: iceblink::ROUND,
    };
    let outer = tx.create_clip(iceblink::ClipNode {
        parent: None,
        space: scene.space,
        shape: shape([0.0, 0.0, 100.0, 100.0]),
    });
    let own = tx.create_clip(iceblink::ClipNode {
        parent: None,
        space: scene.space,
        shape: shape([10.0, 10.0, 50.0, 50.0]),
    });
    let effect = tx.create_effect(EffectNode {
        clip: Some(outer),
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
        isolate: false,
    });
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let key = ChunkKey::new(1, 0);
    let mut clipped = props(scene.space);
    clipped.clip = Some(own);
    tx.record(key, clipped)
        .quad(quad([0.0, 0.0, 80.0, 80.0], PaintRef::NONE));
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(node)]);
    scene.commit(tx);
    scene.entries += 1;
    assert_eq!(scene.draw(), Built::Frame);

    // Frames that draw a new quad elsewhere, and not the clipped chunk.
    let mut part = 0;
    let mut elsewhere = |scene: &mut Scene| {
        for _ in 0..65 {
            part += 1;
            scene.add_chunk(ChunkKey::new(2, part), &[[part as f32, 120.0, 1.0, 1.0]]);
            assert_eq!(scene.draw(), Built::Frame);
        }
    };
    elsewhere(&mut scene);
    assert_eq!(
        common::stat(&scene.compositor, "combined_clips"),
        1,
        "the chunk's lowering holds its chain"
    );

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 1, &[]);
    tx.remove(node);
    tx.remove(effect);
    tx.remove_chunk(key);
    scene.commit(tx);
    scene.entries -= 1;
    elsewhere(&mut scene);
    assert_eq!(common::stat(&scene.compositor, "combined_clips"), 0);
}
