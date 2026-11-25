//! Path items lowered into mono masks or frame paths.

mod common;

use std::sync::Arc;

use common::content::{gradient, solid};
use iceblink::geom::Affine2;
use iceblink::kurbo::{Affine, BezPath, Circle, Rect, Shape, Stroke};
use iceblink::{
    ChunkKey, ClientId, ClipRef, FillRule, Matrix4, PathClip, PathFill, PathItem, PathStroke,
    SpaceRef,
};
use iceblink_core::content::{
    ContentCache, ContentConfig, Lowered, Lowering, PathOwner, PathRoute, Target,
};

/// Returns a filled circle of `radius` at `center`, painted with `paint`.
fn disc(center: (f64, f64), radius: f64, paint: iceblink::PaintRef) -> PathItem {
    PathItem {
        path: Arc::new(Circle::new(center, radius).to_path(0.1)),
        placement: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from([]),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    }
}

/// Lowers `item` at `target` as the path at `position` of one chunk, and returns its route and
/// lowering.
fn lower(
    cache: &mut ContentCache,
    item: &PathItem,
    target: &Target,
    position: u32,
) -> (PathRoute, Lowering) {
    let owner = PathOwner {
        client: ClientId(0),
        chunk: ChunkKey::new(1, 0),
        position,
    };
    let mut out = Lowering::default();
    let route = cache.lower_path(item, target, owner, &mut out);
    (route, out)
}

#[test]
fn a_small_solid_path_becomes_a_mask_on_whole_device_pixels() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let item = disc((10.0, 10.0), 6.0, solid(3));
    let target = Target::new(Matrix4::translation(0.3, 0.6, 0.0));

    let (route, out) = lower(&mut cache, &item, &target, 1);

    assert_eq!(route, PathRoute::Masks);
    match &out.items[..] {
        [Lowered::Mono(sprite)] => {
            assert_eq!(sprite.paint, solid(3));
            assert_eq!((sprite.bounds[0] + 0.3).fract(), 0.0);
            assert_eq!((sprite.bounds[1] + 0.6).fract(), 0.0);
            assert!(sprite.bounds[0] + 0.3 <= 4.3 && sprite.bounds[2] >= 12.0);
        }
        other => panic!("expected one mono sprite, got {other:?}"),
    }
    assert_eq!(out.tiles.len(), 1);
}

#[test]
fn a_whole_pixel_move_reuses_the_mask() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let item = disc((10.0, 10.0), 6.0, solid(3));

    let (_, first) = lower(
        &mut cache,
        &item,
        &Target::new(Matrix4::translation(0.5, 0.0, 0.0)),
        1,
    );
    let (_, moved) = lower(
        &mut cache,
        &item,
        &Target::new(Matrix4::translation(7.5, -3.0, 0.0)),
        1,
    );

    assert_eq!(first.tiles, moved.tiles);
    assert_eq!(cache.stats().masks_rasterised, 1);
}

#[test]
fn a_fill_and_a_stroke_become_two_masks_fill_first() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let mut item = disc((10.0, 10.0), 6.0, solid(3));
    item.stroke = Some(PathStroke {
        paint: solid(4),
        style: Stroke::new(2.0),
    });

    let (route, out) = lower(&mut cache, &item, &Target::new(Matrix4::IDENTITY), 1);

    assert_eq!(route, PathRoute::Masks);
    let paints: Vec<_> = out
        .items
        .iter()
        .map(|item| match item {
            Lowered::Mono(sprite) => sprite.paint,
            other => panic!("expected a mono sprite, got {other:?}"),
        })
        .collect();
    assert_eq!(paints, [solid(3), solid(4)]);
}

#[test]
fn gradients_clips_turns_and_large_paths_stay_paths() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let identity = Target::new(Matrix4::IDENTITY);
    let mut clipped = disc((10.0, 10.0), 6.0, solid(3));
    clipped.clips = Arc::from([PathClip {
        path: Arc::new(Rect::new(0.0, 0.0, 8.0, 8.0).to_path(0.1)),
        rule: FillRule::NON_ZERO,
    }]);
    let turned = Target::new(Affine2::rotation(0.3).to_matrix4());

    let routes = [
        lower(
            &mut cache,
            &disc((10.0, 10.0), 6.0, gradient(3)),
            &identity,
            1,
        )
        .0,
        lower(&mut cache, &clipped, &identity, 2).0,
        lower(&mut cache, &disc((10.0, 10.0), 6.0, solid(3)), &turned, 3).0,
        lower(
            &mut cache,
            &disc((900.0, 900.0), 800.0, solid(3)),
            &identity,
            4,
        )
        .0,
    ];

    assert_eq!(routes, [PathRoute::Path; 4]);
    let (_, out) = lower(&mut cache, &clipped, &identity, 2);
    match &out.items[..] {
        [Lowered::Path(path)] => {
            assert_eq!(path.ink.origin.x, 4);
            assert_eq!(path.ink.size.width, 4, "the ink is cut to the clip");
        }
        other => panic!("expected one path, got {other:?}"),
    }
}

#[test]
fn a_quarter_turn_keeps_the_mask() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let quarter = Target::new(Affine2::new(0.0, 1.0, -1.0, 0.0, 50.0, 0.0).to_matrix4());

    let (route, _) = lower(&mut cache, &disc((10.0, 10.0), 6.0, solid(3)), &quarter, 1);

    assert_eq!(route, PathRoute::Masks);
}

#[test]
fn a_projective_world_maps_the_path_onto_the_device() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let mut world = Matrix4::IDENTITY;
    world.columns[0][3] = 0.001;
    let item = disc((10.0, 10.0), 6.0, gradient(3));

    let (route, out) = lower(&mut cache, &item, &Target::new(world), 1);

    assert_eq!(route, PathRoute::Path);
    match &out.items[..] {
        [Lowered::Path(path)] => {
            assert_eq!(path.transform, Affine::IDENTITY);
            assert!(!Arc::ptr_eq(&path.path, &item.path));
        }
        other => panic!("expected one path, got {other:?}"),
    }
}

#[test]
fn a_path_with_neither_fill_nor_stroke_lowers_to_nothing() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let mut item = disc((1.0, 1.0), 1.0, solid(0));
    item.fill = None;
    item.path = Arc::new(BezPath::new());

    let (route, out) = lower(&mut cache, &item, &Target::new(Matrix4::IDENTITY), 1);

    assert_eq!(route, PathRoute::Nothing);
    assert!(out.items.is_empty());
}

#[test]
fn a_stroke_mask_scales_with_the_placement() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let mut item = disc((10.0, 10.0), 6.0, solid(3));
    item.placement = Affine::scale(2.0);
    item.stroke = Some(PathStroke {
        paint: solid(4),
        style: Stroke::new(2.0),
    });

    let (route, out) = lower(&mut cache, &item, &Target::new(Matrix4::IDENTITY), 1);

    assert_eq!(route, PathRoute::Masks);
    let bounds: Vec<[f32; 4]> = out
        .items
        .iter()
        .map(|item| match item {
            Lowered::Mono(sprite) => sprite.bounds,
            other => panic!("expected a mono sprite, got {other:?}"),
        })
        .collect();
    assert_eq!(bounds, [[8.0, 8.0, 24.0, 24.0], [6.0, 6.0, 28.0, 28.0]]);
}
