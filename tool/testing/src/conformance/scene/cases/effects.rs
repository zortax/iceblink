//! Effects: which effects isolate into groups, what a group holds and carries, and the clip of
//! an effect that does not isolate.

use std::sync::Arc;

use iceblink::{
    Backdrop, BatchKind, BlendMode, ClipEntry, ClipNode, ClipRef, ClipShape, Color, EffectNode,
    Filter, FrameClipShape, FrameFilter, OrderEntry, OrderNode, ParamsBlock, ROUND,
    RenderCapabilities, ShaderDesc, ShaderMode, ShaderSource, SpaceRef, read_extent,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{faded, key, record_quads};
use crate::conformance::scene::stage::{SIDE, Stage};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Places chunk 2 under a node with `effect`, between chunks 1 and 3 in the root.
fn nested(stage: &mut Stage, effect: EffectNode) {
    let mut tx = stage.tx();
    let effect = tx.create_effect(effect);
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([0.0, 0.0, 40.0, 40.0], RED)],
    );
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[([20.0, 20.0, 40.0, 40.0], GREEN)],
    );
    record_quads(
        &mut tx,
        key(3),
        stage.props(),
        &[([30.0, 30.0, 40.0, 40.0], BLUE)],
    );
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(2))]);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.append(&mut tx, OrderEntry::Chunk(key(3)));
    stage.commit(tx);
    stage.draw();
}

/// An effect with an opacity below one, a blend mode or filters isolates its subtree into a group
/// that carries them, and the group's markers enclose exactly the subtree.
pub fn isolating_effects(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    nested(
        &mut stage,
        EffectNode {
            clip: None,
            opacity: 0.5,
            blend: BlendMode::MULTIPLY,
            filters: Arc::from(Vec::new()),
            isolate: false,
        },
    );
    let picture = stage.redraw();
    assert_eq!(picture.groups.len(), 1, "the effect forms one group");
    let group = &picture.groups[0];
    assert_eq!(group.opacity, 0.5, "the group carries the opacity");
    assert_eq!(
        group.blend,
        BlendMode::MULTIPLY,
        "the group carries the blend mode"
    );
    assert_eq!(
        picture.quad(GREEN).groups,
        vec![0],
        "the subtree is drawn inside the group"
    );
    assert!(
        picture.quad(RED).groups.is_empty() && picture.quad(BLUE).groups.is_empty(),
        "content outside the subtree is drawn outside the group"
    );
}

/// A blurred group's source is its content ink grown by what the blur reads, and its bounds are
/// the source cut to the output. Content past the output that the blur reads is drawn.
pub fn group_bounds(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let filters = [Filter::Blur(4.0)];
    nested(
        &mut stage,
        EffectNode {
            clip: None,
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from(filters.to_vec()),
            isolate: false,
        },
    );
    let picture = stage.redraw();
    assert_eq!(picture.groups.len(), 1, "a filtered effect forms one group");
    let group = &picture.groups[0];
    assert_eq!(
        group.filters.as_ref(),
        &[FrameFilter::Blur(4.0)],
        "the group carries the filter"
    );
    let expected = read_extent([20.0, 20.0, 40.0, 40.0], &filters);
    assert_eq!(
        group.bounds, expected,
        "the written region is the content ink grown by the blur's reach"
    );

    assert_eq!(
        group.source, expected,
        "the source of uncut content is its bounds"
    );

    // Content across the output's edge, and a square wholly past it within the blur's reach.
    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[
            ([100.0, 0.0, 28.0, 28.0], GREEN),
            ([130.0, 4.0, 6.0, 6.0], BLUE),
        ],
    );
    stage.commit(tx);
    stage.draw();
    let picture = stage.redraw();
    let group = &picture.groups[0];
    let [x, y, width, height] = group.bounds;
    assert!(
        x >= 0.0 && y >= 0.0 && x + width <= SIDE as f32 && y + height <= SIDE as f32,
        "the written region is cut to the output: {:?}",
        group.bounds
    );
    assert_eq!(
        group.source,
        read_extent([100.0, 0.0, 36.0, 28.0], &filters),
        "the source is the content ink grown by the blur's reach, past the output"
    );
    assert!(
        picture.quads_of(BLUE).iter().any(|quad| quad.groups == [0]),
        "the square past the output is drawn into the group"
    );
}

/// An effect that does not isolate draws its subtree in place, and its clip joins the chain of
/// every item of the subtree.
pub fn effects_in_place(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [24.0, 24.0, 20.0, 20.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    stage.commit(tx);
    nested(
        &mut stage,
        EffectNode {
            clip: Some(clip),
            ..faded(1.0)
        },
    );
    let picture = stage.redraw();
    assert!(
        picture.groups.is_empty(),
        "an effect that does not isolate forms no group"
    );
    let green = picture.quad(GREEN);
    assert!(
        has_rect(&green.clip, [24.0, 24.0, 20.0, 20.0]),
        "the effect's clip joins the item's chain: {:?}",
        green.clip
    );
}

/// The clip of an effect that does not isolate cuts the composite of a group nested under it,
/// and neither the content the group's filters read nor the group's bounds.
pub fn clips_above_groups(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let window = [24.0, 24.0, 20.0, 20.0];
    let mut tx = stage.tx();
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: window,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let outer = tx.create_effect(EffectNode {
        clip: Some(clip),
        ..faded(1.0)
    });
    let inner = tx.create_effect(EffectNode {
        filters: Arc::from([Filter::Blur(2.0)]),
        ..faded(1.0)
    });
    let cut = tx.create_order(OrderNode {
        effect: Some(outer),
    });
    let blurred = tx.create_order(OrderNode {
        effect: Some(inner),
    });
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[([20.0, 20.0, 40.0, 40.0], GREEN)],
    );
    tx.splice(blurred, 0, 0, &[OrderEntry::Chunk(key(2))]);
    tx.splice(cut, 0, 0, &[OrderEntry::Node(blurred)]);
    stage.append(&mut tx, OrderEntry::Node(cut));
    stage.commit(tx);
    let picture = stage.redraw();

    let green = picture.quad(GREEN);
    assert!(
        !has_rect(&green.clip, window),
        "a quad inside a group is drawn without the clip of the effect above the group: {:?}",
        green.clip
    );
    assert!(
        picture.group_clips.len() == 1 && has_rect(&picture.group_clips[0], window),
        "the group is composited through the clip of the effect above it: {:?}",
        picture.group_clips
    );
    let [x, y, width, height] = picture.groups[0].bounds;
    assert!(
        x <= 20.0 && y <= 20.0 && x + width >= 60.0 && y + height >= 60.0,
        "the group's bounds hold content the clip above cuts away, for its blur to read: {:?}",
        picture.groups[0].bounds
    );
}

/// A group whose content ink is empty draws nothing whatever its filters, and a backdrop's
/// rectangles are its device bounding box and read extent, neither rounded nor cut.
pub fn empty_groups_and_backdrops(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let shadowed = tx.create_effect(EffectNode {
        filters: Arc::from([Filter::DropShadow {
            offset: [4.0, 4.0],
            std_dev: 3.0,
            color: Color([0.0, 0.0, 0.0, 1.0]),
        }]),
        ..faded(1.0)
    });
    let node = tx.create_order(OrderNode {
        effect: Some(shadowed),
    });
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(9))]);
    stage.append(&mut tx, OrderEntry::Node(node));

    let blur = [Filter::Blur(2.0)];
    let bounds = [100.5, 10.25, 50.0, 20.0];
    let mut chunk = tx.record(key(2), stage.props());
    let chain = chunk.filters(Arc::from(blur));
    chunk.backdrop(Backdrop {
        bounds,
        filters: chain,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(2)));
    stage.commit(tx);
    stage.redraw();

    let opens = stage
        .frame
        .batches
        .iter()
        .any(|batch| matches!(batch.kind, BatchKind::GroupBegin(_)));
    assert!(!opens, "a group with no content draws nothing");
    let backdrop = stage.frame.backdrops.first();
    assert_eq!(
        backdrop.map(|backdrop| (backdrop.bounds, backdrop.source)),
        Some((bounds, read_extent(bounds, &blur))),
        "a backdrop's rectangles are exact and reach past the output"
    );
}

/// For a renderer without custom shaders, a group's chain leaves a shader filter out or refers to a
/// shader the renderer was sent, and the group's content is drawn.
pub fn shader_filters_without_custom_shaders(make: MakeCompositor<'_>) {
    let mut stage = Stage::new(make, RenderCapabilities::MINIMAL);
    let mut tx = stage.tx();
    let shader = tx.create_shader(ShaderDesc {
        mode: ShaderMode::FILTER,
        source: ShaderSource {
            wgsl: Arc::from(""),
            prebuilt: None,
        },
        params_size: 0,
        reads: Default::default(),
    });
    let params = tx.create_params(ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    let effect = tx.create_effect(EffectNode {
        filters: Arc::from([Filter::Shader {
            shader,
            params,
            reach: 4.0,
        }]),
        ..faded(1.0)
    });
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([20.0, 20.0, 20.0, 20.0], GREEN)],
    );
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(1))]);
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.commit(tx);
    // The frame check of every frame holds the chain to the shaders the renderer was sent.
    let picture = stage.redraw();
    assert_eq!(
        picture.quads_of(GREEN).len(),
        1,
        "the group's content is drawn"
    );
}

/// Reports whether `entry` holds a rounded rectangle link over `rect`.
fn has_rect(entry: &Option<ClipEntry>, rect: [f32; 4]) -> bool {
    entry.as_ref().is_some_and(|entry| {
        entry.links.iter().any(|link| {
            matches!(link.shape, FrameClipShape::RoundedRect { rect: linked, .. } if linked == rect)
        })
    })
}
