//! One pass as part of the scene of its scratch layer.

use iceblink::{FillRule, FramePaint, FramePath, PlannedPath};
use iceblink_wgpu::vector::paint::resolve;
use iceblink_wgpu::vector::{ClipOutline, ResourcedPass, VectorFrame};
use kurbo::Affine;
use smallvec::SmallVec;
use vello::Scene;

use super::cached::{Encodings, Fingerprint};
use super::paint;

/// The counts of what encoding cost and could not do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Encoded {
    /// Clip layers pushed for residual clips and path clips.
    pub clip_layers: u32,
    /// Paths left undrawn because a residual clip link has no outline.
    pub unclippable: u32,
    /// Paths left undrawn because neither their fill nor their stroke paints anything.
    pub unpaintable: u32,
}

impl Encoded {
    /// Adds the counts of `other`.
    pub(crate) fn add(&mut self, other: Self) {
        self.clip_layers += other.clip_layers;
        self.unclippable += other.unclippable;
        self.unpaintable += other.unpaintable;
    }
}

/// Appends the paths of `pass` to `scene`, moved into the pass's bin.
///
/// The scene is not reset, since every pass of one layer is rasterised in one scene, and their
/// bins do not meet. A clamped pass is clipped to its bin. `stops` is scratch for gradient stops.
pub(crate) fn pass(
    scene: &mut Scene,
    cache: &mut Encodings,
    frame: &VectorFrame<'_>,
    pass: &ResourcedPass,
    stops: &mut Vec<(f32, [f32; 4])>,
) -> Encoded {
    let mut encoded = Encoded::default();
    let shift = Affine::translate((
        f64::from(pass.raster_region.origin.x - pass.region.origin.x),
        f64::from(pass.raster_region.origin.y - pass.region.origin.y),
    ));
    if pass.clamped {
        let bin = pass.raster_region;
        let bin = kurbo::Rect::new(
            f64::from(bin.left()),
            f64::from(bin.top()),
            f64::from(bin.right()),
            f64::from(bin.bottom()),
        );
        scene.push_clip_layer(peniko::Fill::NonZero, Affine::IDENTITY, &bin);
    }

    for planned in frame.plan.items_of(pass) {
        encoded.add(clipped_path(scene, cache, frame, planned, shift, stops));
    }

    if pass.clamped {
        scene.pop_layer();
    }
    encoded
}

/// Appends the path of `planned` to `scene` under `shift`, inside its residual clip and its own
/// clips.
///
/// Nothing is appended for a planned item without a path, or for a path whose residual clip has
/// a link without an outline. The second counts as unclippable.
fn clipped_path(
    scene: &mut Scene,
    cache: &mut Encodings,
    frame: &VectorFrame<'_>,
    planned: &PlannedPath,
    shift: Affine,
    stops: &mut Vec<(f32, [f32; 4])>,
) -> Encoded {
    let mut encoded = Encoded::default();
    let Some(path) = frame.path(planned) else {
        return encoded;
    };
    let Some(residual) = frame
        .residual(planned)
        .iter()
        .map(|link| ClipOutline::of(link, frame.spaces))
        .collect::<Option<SmallVec<[ClipOutline; 2]>>>()
    else {
        encoded.unclippable += 1;
        return encoded;
    };

    // The residual is part of the path's clip chain, so each link goes through its own space.
    for outline in &residual {
        scene.push_clip_layer(
            peniko::Fill::NonZero,
            shift * outline.transform,
            &outline.path,
        );
        encoded.clip_layers += 1;
    }
    // The path's own clips are in path space and go through the path's transform.
    let placement = shift * path.transform;
    for clip in path.clips.iter() {
        scene.push_clip_layer(fill_rule(clip.rule), placement, clip.path.as_ref());
        encoded.clip_layers += 1;
    }

    if !place(scene, cache, path, frame.paints, placement, stops) {
        encoded.unpaintable += 1;
    }

    for _ in 0..residual.len() + path.clips.len() {
        scene.pop_layer();
    }
    encoded
}

/// Appends the encoding of `path` under `placement`, and reports whether anything is painted.
///
/// The encoding is made in path space on a cache miss, with the fill drawn first and the stroke
/// over it.
fn place(
    scene: &mut Scene,
    cache: &mut Encodings,
    path: &FramePath,
    paints: &[FramePaint],
    placement: Affine,
    stops: &mut Vec<(f32, [f32; 4])>,
) -> bool {
    let painting =
        |reference| resolve(reference, paints).filter(|paint| **paint != FramePaint::None);
    let fill = path
        .fill
        .and_then(|fill| painting(fill.paint).map(|paint| (fill.rule, paint)));
    let stroke = path
        .stroke
        .as_ref()
        .and_then(|stroke| painting(stroke.paint).map(|paint| (&stroke.style, paint)));
    if fill.is_none() && stroke.is_none() {
        return false;
    }

    let wanted = Fingerprint::of(
        path,
        fill.map(|(_, paint)| paint),
        stroke.map(|(_, paint)| paint),
    );
    let encoding = cache.get(wanted, &path.path, |into| {
        if let Some((rule, paint)) = fill
            && let Some(painted) = paint::brush(paint, stops)
        {
            let brush = brush_transform(path.brush, painted.transform);
            into.fill(
                fill_rule(rule),
                Affine::IDENTITY,
                &painted.brush,
                brush,
                path.path.as_ref(),
            );
        }
        if let Some((style, paint)) = stroke
            && let Some(painted) = paint::brush(paint, stops)
        {
            let brush = brush_transform(path.brush, painted.transform);
            into.stroke(
                style,
                Affine::IDENTITY,
                &painted.brush,
                brush,
                path.path.as_ref(),
            );
        }
    });
    scene.append(encoding, Some(placement));
    true
}

/// Returns the map from brush space into path space: the path's brush after the paint's own
/// transform, or `None` for the identity.
fn brush_transform(brush: Affine, paint: Option<Affine>) -> Option<Affine> {
    let paint = paint.unwrap_or(Affine::IDENTITY);
    (brush != Affine::IDENTITY || paint != Affine::IDENTITY).then_some(brush * paint)
}

/// Returns the path renderer's fill rule for `rule`.
fn fill_rule(rule: FillRule) -> peniko::Fill {
    if rule == FillRule::EVEN_ODD {
        peniko::Fill::EvenOdd
    } else {
        peniko::Fill::NonZero
    }
}
