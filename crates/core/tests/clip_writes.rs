//! Clip writes drawn into a kept target on a device, compared with the same pictures redrawn
//! whole.

mod common;

use std::sync::Arc;

use iceblink::geom::Affine2;
use iceblink::kurbo::{Affine, Circle, Shape};
use iceblink::{
    Backdrop, BlendMode, ChunkKey, ClipId, ClipNode, ClipRef, ClipShape, Color, CornerShapes,
    EffectNode, FillRule, Filter, LocalClip, LocalParent, Matrix4, OrderEntry, OrderId, OrderNode,
    Paint, PaintId, PaintKind, PathFill, PathItem, Quad, ROUND, SpaceRef, Transaction, TransformId,
};
use iceblink_testing::gpu::device_lock;

use common::story::World;
use common::text::run;
use common::twin::Twin;
use common::{identity, props, quad};

/// The ids a scene's edits write.
#[derive(Clone, Copy)]
struct Ids {
    outer: ClipId,
    inner: ClipId,
    /// The space `outer` is measured in.
    space: TransformId,
    /// The paint of the quad that crosses the clips' edges.
    crossing: PaintId,
}

/// Returns a clip node under `parent` over `rect` in `space` with `radii` and `shapes`.
fn rounded(
    space: TransformId,
    parent: Option<ClipId>,
    rect: [f32; 4],
    radii: [f32; 8],
    shapes: CornerShapes,
) -> ClipNode {
    ClipNode {
        parent,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        },
    }
}

/// Returns a clip node under `parent` over `rect` in `space`, without radii.
fn square(space: TransformId, parent: Option<ClipId>, rect: [f32; 4]) -> ClipNode {
    rounded(space, parent, rect, [0.0; 8], ROUND)
}

/// Records chunk `key` with an opaque quad, a translucent rounded quad, a quad crossing the
/// output, a glyph run, a path and a blurred backdrop, all drawn through `clip`, and puts it
/// first in `parent`.
fn content(
    tx: &mut Transaction,
    world: &mut World,
    (key, parent): (ChunkKey, OrderId),
    clip: Option<ClipId>,
    crossing: PaintId,
) {
    let green = world.solid(tx, [0.1, 0.6, 0.2, 1.0]);
    let violet = world.solid(tx, [0.3, 0.1, 0.4, 0.6]);
    let mut chunk_props = props(world.space);
    chunk_props.clip = clip;
    let mut chunk = tx.record(key, chunk_props);
    let fill = chunk.paint_ref(PaintKind::SOLID, green);
    chunk.quad(quad([4.0, 4.0, 120.0, 120.0], fill));
    let fill = chunk.paint_ref(PaintKind::SOLID, violet);
    chunk.quad(Quad {
        radii: [12.0; 8],
        ..quad([20.25, 16.5, 70.0, 90.0], fill)
    });
    let fill = chunk.paint_ref(PaintKind::SOLID, crossing);
    chunk.quad(quad([0.0, 50.0, 128.0, 12.0], fill));
    let ink = chunk.paint_ref(PaintKind::SOLID, violet);
    run(&mut chunk, world.font, "Clip", 18.0, [24.0, 40.0], ink);
    chunk.path(PathItem {
        path: Arc::new(Circle::new((90.0, 92.0), 16.0).to_path(0.1)),
        placement: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint: fill,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    let chain = chunk.filters(Arc::from(vec![Filter::Blur(2.0)]));
    chunk.backdrop(Backdrop {
        bounds: [36.0, 66.0, 50.0, 30.0],
        filters: chain,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    tx.splice(parent, 0, 0, &[OrderEntry::Chunk(key)]);
}

/// Commits a white backing and the content of [`content`], drawn through a square clip over
/// `inner`, and returns the ids.
///
/// The square clip is a child of the clip `outer` returns for a space of its own.
fn scene(twin: &mut Twin, outer: impl Fn(TransformId) -> ClipNode, inner: [f32; 4]) -> Ids {
    twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let space = tx.create_transform(identity(Some(world.space)));
        let outer = tx.create_clip(outer(space));
        let inner = tx.create_clip(square(world.space, Some(outer), inner));
        let crossing = world.solid(tx, [0.9, 0.4, 0.1, 1.0]);
        let list = tx.create_order(OrderNode::default());
        world.append(tx, OrderEntry::Node(list));
        content(
            tx,
            world,
            (ChunkKey::new(2, 0), list),
            Some(inner),
            crossing,
        );
        Ids {
            outer,
            inner,
            space,
            crossing,
        }
    })
}

/// An edit written into a transaction.
type Edit<'a> = &'a dyn Fn(&mut Transaction, Ids);

/// Commits each edit in its own transaction and checks every frame against a whole redraw.
fn check(twin: &mut Twin, ids: Ids, edits: &[(&str, Edit<'_>)]) {
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");
    for (name, edit) in edits {
        twin.commit(&mut (), |tx, _, _| edit(tx, ids));
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "{name} differs: {found:?}");
    }
}

#[test]
fn moved_square_edges_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scene(
        &mut twin,
        |space| square(space, None, [8.0, 8.0, 100.0, 100.0]),
        [0.0, 0.0, 128.0, 128.0],
    );
    let outer = |rect: [f32; 4]| {
        move |tx: &mut Transaction, ids: Ids| {
            tx.set_clip(ids.outer, square(ids.space, None, rect));
        }
    };
    check(
        &mut twin,
        ids,
        &[
            ("the bottom edge up", &outer([8.0, 8.0, 100.0, 95.0])),
            (
                "the bottom edge to a fraction",
                &outer([8.0, 8.0, 100.0, 92.4]),
            ),
            ("the left edge in", &outer([20.5, 8.0, 87.5, 92.4])),
            ("every edge out", &outer([2.0, 3.0, 120.0, 121.0])),
            ("past every item", &outer([0.0, 0.0, 128.0, 128.0])),
            ("to a thin column", &outer([60.3, 0.0, 0.6, 128.0])),
            ("back", &outer([8.0, 8.0, 100.0, 100.0])),
        ],
    );
}

#[test]
fn rounded_corners_that_change_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let rect = [8.0, 8.0, 100.0, 100.0];
    let ids = scene(
        &mut twin,
        |space| rounded(space, None, rect, [10.0; 8], ROUND),
        [0.0, 0.0, 128.0, 128.0],
    );
    let outer = |rect: [f32; 4], radii: [f32; 8], shapes: CornerShapes| {
        move |tx: &mut Transaction, ids: Ids| {
            tx.set_clip(ids.outer, rounded(ids.space, None, rect, radii, shapes));
        }
    };
    let top_left = |radius: f32| {
        let mut radii = [10.0; 8];
        radii[0] = radius;
        radii[1] = radius * 1.5;
        radii
    };
    check(
        &mut twin,
        ids,
        &[
            (
                "a larger top left corner",
                &outer(rect, top_left(20.0), ROUND),
            ),
            ("squircle corners", &outer(rect, top_left(20.0), [4.0; 4])),
            (
                "scooped corners",
                &outer(rect, top_left(20.0), [0.6, 2.0, 2.0, 2.0]),
            ),
            (
                "a narrower clip",
                &outer([8.0, 8.0, 70.0, 100.0], [10.0; 8], ROUND),
            ),
            (
                "a lower top",
                &outer([8.0, 30.0, 70.0, 78.0], [30.0; 8], ROUND),
            ),
            (
                "an uneven corner",
                &outer(
                    [8.0, 30.0, 70.0, 78.0],
                    [5.0, 30.0, 30.0, 5.0, 10.0, 20.0, 40.0, 8.0],
                    ROUND,
                ),
            ),
            (
                "square again",
                &outer([8.0, 30.0, 70.0, 78.0], [0.0; 8], ROUND),
            ),
            ("rounded again", &outer(rect, [10.0; 8], ROUND)),
        ],
    );
}

#[test]
fn nested_clips_that_change_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scene(
        &mut twin,
        |space| square(space, None, [8.0, 8.0, 100.0, 100.0]),
        [20.0, 0.0, 60.0, 128.0],
    );
    check(
        &mut twin,
        ids,
        &[
            ("the outer clip", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [8.0, 8.0, 100.0, 60.0]));
            }),
            ("the inner clip", &|tx, ids| {
                let inner = rounded(
                    ids.space,
                    Some(ids.outer),
                    [10.0, 4.0, 60.0, 120.0],
                    [8.0; 8],
                    ROUND,
                );
                tx.set_clip(ids.inner, inner);
            }),
            ("both", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [8.0, 20.0, 100.0, 80.0]));
                tx.set_clip(
                    ids.inner,
                    square(ids.space, Some(ids.outer), [30.0, 4.0, 60.0, 120.0]),
                );
            }),
            ("the inner clip without its parent", &|tx, ids| {
                tx.set_clip(ids.inner, square(ids.space, None, [30.0, 4.0, 60.0, 120.0]));
            }),
        ],
    );
}

#[test]
fn clips_in_moved_spaces_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scene(
        &mut twin,
        |space| rounded(space, None, [8.0, 8.0, 80.0, 80.0], [6.0; 8], ROUND),
        [0.0, 0.0, 128.0, 128.0],
    );
    let turned = Affine2::rotation(0.3)
        .then(Affine2::translation(30.0, 0.0))
        .to_matrix4();
    check(
        &mut twin,
        ids,
        &[
            ("a move", &|tx, ids| {
                tx.set_transform(ids.space, Matrix4::translation(10.0, 4.5, 0.0));
            }),
            ("a fractional move", &|tx, ids| {
                tx.set_transform(ids.space, Matrix4::translation(10.3, 4.7, 0.0));
            }),
            ("a scale", &|tx, ids| {
                let scaled =
                    Matrix4::scale(1.25, 0.8, 1.0).then(&Matrix4::translation(4.0, 6.0, 0.0));
                tx.set_transform(ids.space, scaled);
            }),
            ("a mirror", &|tx, ids| {
                let mirrored =
                    Matrix4::scale(-1.0, 1.0, 1.0).then(&Matrix4::translation(120.0, 0.0, 0.0));
                tx.set_transform(ids.space, mirrored);
            }),
            ("a mirrored write", &|tx, ids| {
                let mut radii = [6.0; 8];
                radii[0] = 20.0;
                let node = rounded(ids.space, None, [8.0, 8.0, 60.0, 80.0], radii, ROUND);
                tx.set_clip(ids.outer, node);
            }),
            ("a turn", &move |tx, ids| {
                tx.set_transform(ids.space, turned)
            }),
            ("a square clip under the turn", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [8.0, 8.0, 80.0, 80.0]));
            }),
            ("its edges under the turn", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [12.0, 8.0, 70.5, 60.0]));
            }),
            ("a rounded clip under the turn", &|tx, ids| {
                let node = rounded(ids.space, None, [12.0, 8.0, 70.5, 60.0], [9.0; 8], ROUND);
                tx.set_clip(ids.outer, node);
            }),
            ("back", &|tx, ids| {
                tx.set_transform(ids.space, Matrix4::IDENTITY)
            }),
        ],
    );
}

#[test]
fn a_clip_written_to_no_visible_change_then_repainted_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scene(
        &mut twin,
        |space| square(space, None, [0.0, 0.0, 128.0, 128.0]),
        [0.0, 0.0, 128.0, 128.0],
    );
    check(
        &mut twin,
        ids,
        &[
            ("a cut past every item", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [0.0, 0.0, 126.0, 126.0]));
            }),
            ("a cut through the crossing quad", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [0.0, 0.0, 100.0, 126.0]));
            }),
            ("a repaint of the crossing quad", &|tx, ids| {
                tx.set_paint(ids.crossing, Paint::Solid(Color([0.1, 0.3, 0.9, 1.0])));
            }),
        ],
    );
}

#[test]
fn effect_clips_that_change_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let (clip, crossing) = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let clip = tx.create_clip(square(world.space, None, [16.0, 16.0, 90.0, 90.0]));
        let crossing = world.solid(tx, [0.9, 0.4, 0.1, 1.0]);
        let effects = [
            EffectNode {
                clip: Some(clip),
                opacity: 0.8,
                blend: BlendMode::NORMAL,
                filters: Arc::from(vec![Filter::Blur(2.0)]),
                isolate: true,
            },
            EffectNode {
                clip: Some(clip),
                opacity: 1.0,
                blend: BlendMode::NORMAL,
                filters: Arc::from(Vec::new()),
                isolate: false,
            },
        ];
        for (part, effect) in effects.into_iter().enumerate() {
            let effect = tx.create_effect(effect);
            let group = tx.create_order(OrderNode {
                effect: Some(effect),
            });
            world.append(tx, OrderEntry::Node(group));
            let key = ChunkKey::new(2, part as u32);
            content(tx, world, (key, group), None, crossing);
        }
        (clip, crossing)
    });
    let ids = Ids {
        outer: clip,
        inner: clip,
        space: twin.kept.world.space,
        crossing,
    };
    let edit = |rect: [f32; 4], radii: [f32; 8]| {
        move |tx: &mut Transaction, ids: Ids| {
            tx.set_clip(ids.outer, rounded(ids.space, None, rect, radii, ROUND));
        }
    };
    check(
        &mut twin,
        ids,
        &[
            ("a lower edge", &edit([16.0, 16.0, 90.0, 60.0], [0.0; 8])),
            ("a wider clip", &edit([4.0, 16.0, 110.0, 60.0], [0.0; 8])),
            ("rounded", &edit([4.0, 16.0, 110.0, 60.0], [12.0; 8])),
            (
                "rounded and narrower",
                &edit([30.0, 10.0, 60.0, 60.0], [12.0; 8]),
            ),
        ],
    );
}

#[test]
fn local_clips_under_a_changed_clip_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let (outer, crossing) = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let outer = tx.create_clip(square(world.space, None, [8.0, 8.0, 100.0, 100.0]));
        let crossing = world.solid(tx, [0.9, 0.4, 0.1, 1.0]);
        let key = ChunkKey::new(2, 0);
        let mut chunk = tx.record(key, props(world.space));
        let local = chunk.local_clip(LocalClip {
            parent: LocalParent::Node(outer),
            shape: ClipShape::RoundedRect {
                rect: [20.0, 0.0, 90.0, 128.0],
                radii: [10.0; 8],
                shapes: ROUND,
            },
        });
        let fill = chunk.paint_ref(PaintKind::SOLID, crossing);
        chunk.quad(Quad {
            clip: local,
            ..quad([0.0, 20.0, 128.0, 90.0], fill)
        });
        tx.splice(world.root, world.entries, 0, &[OrderEntry::Chunk(key)]);
        world.entries += 1;
        (outer, crossing)
    });
    let ids = Ids {
        outer,
        inner: outer,
        space: twin.kept.world.space,
        crossing,
    };
    check(
        &mut twin,
        ids,
        &[
            ("the outer clip", &|tx, ids| {
                tx.set_clip(ids.outer, square(ids.space, None, [8.0, 30.0, 70.0, 60.0]));
            }),
            ("the outer clip rounded", &|tx, ids| {
                let node = rounded(ids.space, None, [8.0, 30.0, 70.0, 60.0], [15.0; 8], ROUND);
                tx.set_clip(ids.outer, node);
            }),
        ],
    );
}
