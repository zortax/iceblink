//! Small solid paths drawn as path masks: mono sprites over their coverage on atlas pages.

use iceblink::geom::Affine2;
use iceblink::kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Rect, Stroke};
use iceblink::{
    AtlasTile, BatchKind, FillRule, PaintKind, PaintRef, PathFill, PathItem, PathStroke, Sprite,
};

use crate::naive::content::{Content, Mask, MaskCommand, MaskStyle, StrokeCap, StrokeJoin};
use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::Placed;
use crate::naive::frame::lower::path::reach;

/// The widest a mask may be.
const MAX_WIDTH: f32 = 1024.0;

/// The tallest a mask may be.
const MAX_HEIGHT: f32 = 256.0;

/// The most texels a mask may cover.
const MAX_TEXELS: f32 = 65536.0;

/// The largest off-axis coefficient of the world matrix, relative to its axis's scale, that
/// still keeps the axes, and the largest relative difference of two scales that are alike.
const EPSILON: f32 = 1.0e-4;

/// The smallest scale of a world matrix axis.
const MIN_DENSITY: f32 = 1.0e-4;

/// The masks of one path item and where they lie.
pub(super) struct MaskPlan {
    /// Mask texels per local unit along each axis.
    density: [f32; 2],
    /// The sub-pixel shift that puts mask texels on whole device pixels.
    phase: [f32; 2],
    /// The fill's and the stroke's mask.
    parts: [Option<PlannedMask>; 2],
}

/// One part's mask, its corner in mask space and its frame paint.
struct PlannedMask {
    mask: Mask,
    corner: [f64; 2],
    paint: PaintRef,
}

/// How one part's outline is drawn, before it is measured in texels.
enum Outline<'a> {
    Fill(FillRule),
    Stroke(&'a Stroke),
}

impl MaskPlan {
    /// Returns the masks of `item` placed by `placed`, with the frame paints `fill` and
    /// `stroke`, or `None` when the path is drawn as a frame path.
    ///
    /// The path has masks when all of these hold:
    ///
    /// 1. It has a fill or a stroke, and no path clips.
    /// 2. Its paints are solid.
    /// 3. A stroke's placement scales both axes alike.
    /// 4. The world matrix is two-dimensional, keeps the axes or turns them by quarter turns, and
    ///    scales both axes alike when there is a stroke.
    /// 5. Each part's mask is at least one texel and at most 1024 by 256 texels and 65536 texels
    ///    in all, with finite points and a stroke of positive finite lengths.
    pub(super) fn new(
        item: &PathItem,
        fill: Option<PathFill>,
        stroke: Option<&PathStroke>,
        placed: &Placed,
    ) -> Option<Self> {
        let solid = |paint: PaintRef| paint.kind == PaintKind::SOLID;
        if !item.clips.is_empty()
            || (fill.is_none() && stroke.is_none())
            || fill.is_some_and(|fill| !solid(fill.paint))
            || stroke.is_some_and(|stroke| !solid(stroke.paint))
        {
            return None;
        }

        let stroked = stroke.is_some();
        if stroked && !alike(scales(item.placement)) {
            return None;
        }
        let world = placed.world.to_affine2()?;
        let density = density(world, stroked)?;
        let mut plan = Self {
            density,
            phase: phase(world),
            parts: [None, None],
        };

        let [x, y] = placed.offset.map(f64::from);
        let placement = Affine::translate((x, y)) * item.placement;
        let bounds = placement.transform_rect_bbox(item.path.control_box());
        if let Some(fill) = fill {
            let mask = plan.mask(&item.path, placement, Outline::Fill(fill.rule), bounds)?;
            plan.parts[0] = Some(PlannedMask {
                paint: fill.paint,
                ..mask
            });
        }
        if let Some(stroke) = stroke {
            // Stroke lengths are in path space, which the placement scales.
            let scale = scales(item.placement)[0];
            let style = scaled(&stroke.style, scale);
            let outset = reach(&stroke.style) * scale;
            let local = bounds.inflate(outset, outset);
            let mask = plan.mask(&item.path, placement, Outline::Stroke(&style), local)?;
            plan.parts[1] = Some(PlannedMask {
                paint: stroke.paint,
                ..mask
            });
        }
        Some(plan)
    }

    /// Returns the fill's and the stroke's mask.
    pub(super) fn masks(&self) -> [Option<&Mask>; 2] {
        self.parts
            .each_ref()
            .map(|part| part.as_ref().map(|part| &part.mask))
    }

    /// Draws the masks as mono sprites, the fill first, and reports whether every mask found
    /// room on a page.
    ///
    /// Nothing is drawn when one did not.
    pub(super) fn draw(&self, builder: &mut Builder<'_>, placed: &Placed) -> bool {
        let side = Content::shared_side(builder.env.caps.max_texture_size);
        let content = &mut *builder.content;
        let mut tiles: [Option<AtlasTile>; 2] = [None, None];
        for (tile, part) in tiles.iter_mut().zip(&self.parts) {
            if let Some(part) = part {
                let Some(placed) = content.masks.tile(&mut content.pages, side, &part.mask) else {
                    return false;
                };
                *tile = Some(placed);
            }
        }

        for (tile, part) in tiles.into_iter().zip(&self.parts) {
            if let (Some(tile), Some(part)) = (tile, part) {
                builder.pages.insert(tile.page);
                let sprite = self.sprite(part, tile, placed);
                builder.instance(
                    |lanes| &mut lanes.mono,
                    sprite,
                    BatchKind::Mono { page: tile.page },
                );
            }
        }
        true
    }

    /// Returns the mask of an outline of `path` whose ink in local space is `local`, or `None`
    /// when the mask is too small, too large or not finite.
    ///
    /// The paint is left for the caller to set.
    fn mask(
        &self,
        path: &BezPath,
        placement: Affine,
        outline: Outline<'_>,
        local: Rect,
    ) -> Option<PlannedMask> {
        let [kx, ky] = self.density.map(f64::from);
        let [px, py] = self.phase.map(f64::from);
        let left = (local.x0 * kx + px).floor();
        let top = (local.y0 * ky + py).floor();
        let right = (local.x1 * kx + px).ceil();
        let bottom = (local.y1 * ky + py).ceil();
        if ![left, top, right, bottom]
            .iter()
            .all(|edge| edge.is_finite())
        {
            return None;
        }

        let (width, height) = ((right - left) as f32, (bottom - top) as f32);
        let fits = width >= 1.0
            && height >= 1.0
            && width <= MAX_WIDTH
            && height <= MAX_HEIGHT
            && width * height <= MAX_TEXELS;
        if !fits {
            return None;
        }

        let style = match outline {
            Outline::Fill(rule) => MaskStyle::Fill {
                even_odd: rule == FillRule::EVEN_ODD,
            },
            Outline::Stroke(stroke) => stroke_style(stroke, self.density[0])?,
        };
        // The corner is an integer texel, which is what the outline is measured from.
        let corner = [left as i32, top as i32];
        let mask = Mask {
            commands: self.commands(path, placement, corner)?,
            style,
            size: [width as u32, height as u32],
        };
        Some(PlannedMask {
            mask,
            corner: [left, top],
            paint: PaintRef::NONE,
        })
    }

    /// Returns the commands of `path` in mask texels from `corner`: placed by `placement`, scaled
    /// by the density and moved by the phase, or `None` when a point is not finite.
    fn commands(
        &self,
        path: &BezPath,
        placement: Affine,
        corner: [i32; 2],
    ) -> Option<Vec<MaskCommand>> {
        let [kx, ky] = self.density.map(f64::from);
        let x = f64::from(corner[0]) - f64::from(self.phase[0]);
        let y = f64::from(corner[1]) - f64::from(self.phase[1]);
        let point = |point: Point| {
            let point = placement * point;
            let texel = [(point.x * kx - x) as f32, (point.y * ky - y) as f32];
            (texel[0].is_finite() && texel[1].is_finite())
                .then_some([texel[0].to_bits(), texel[1].to_bits()])
        };

        path.elements()
            .iter()
            .map(|element| match *element {
                PathEl::MoveTo(p) => Some(MaskCommand::Move(point(p)?)),
                PathEl::LineTo(p) => Some(MaskCommand::Line(point(p)?)),
                PathEl::QuadTo(a, b) => {
                    let [a, b] = [point(a)?, point(b)?];
                    Some(MaskCommand::Quad([a[0], a[1], b[0], b[1]]))
                }
                PathEl::CurveTo(a, b, c) => {
                    let [a, b, c] = [point(a)?, point(b)?, point(c)?];
                    Some(MaskCommand::Curve([a[0], a[1], b[0], b[1], c[0], c[1]]))
                }
                PathEl::ClosePath => Some(MaskCommand::Close),
            })
            .collect()
    }

    /// Returns the sprite that draws `part` from `tile`.
    fn sprite(&self, part: &PlannedMask, tile: AtlasTile, placed: &Placed) -> Sprite {
        let [kx, ky] = self.density.map(f64::from);
        let [px, py] = self.phase.map(f64::from);
        let [left, top] = part.corner;
        let [width, height] = part.mask.size.map(|side| side as f32);
        Sprite {
            bounds: [
                ((left - px) / kx) as f32,
                ((top - py) / ky) as f32,
                width / self.density[0],
                height / self.density[1],
            ],
            paint: part.paint,
            paint_origin: placed.offset,
            tile,
            clip: placed.clip,
            space: placed.space,
            _pad: 0,
        }
    }
}

/// Returns the mask texels per local unit along each axis of `world`, or `None` when an axis's
/// scale is not finite or below [`MIN_DENSITY`], or the matrix neither keeps the axes nor turns
/// them by quarter turns, or a `stroked` path's scales are not alike.
fn density(world: Affine2, stroked: bool) -> Option<[f32; 2]> {
    let kx = world.a.hypot(world.b);
    let ky = world.c.hypot(world.d);
    if !kx.is_finite() || !ky.is_finite() || kx < MIN_DENSITY || ky < MIN_DENSITY {
        return None;
    }

    let axial = world.b.abs() <= EPSILON * kx && world.c.abs() <= EPSILON * ky;
    let quarter = world.a.abs() <= EPSILON * kx && world.d.abs() <= EPSILON * ky;
    if !axial && !quarter {
        return None;
    }
    if stroked && (kx - ky).abs() > EPSILON * kx.max(ky) {
        return None;
    }
    Some([kx, ky])
}

/// Returns the fractions of the translation of `world` when it neither turns nor flips, and zero
/// otherwise.
fn phase(world: Affine2) -> [f32; 2] {
    if world.b == 0.0 && world.c == 0.0 && world.a > 0.0 && world.d > 0.0 {
        [world.tx - world.tx.floor(), world.ty - world.ty.floor()]
    } else {
        [0.0, 0.0]
    }
}

/// Returns the scales of the two axes of `placement`.
fn scales(placement: Affine) -> [f64; 2] {
    let [a, b, c, d, _, _] = placement.as_coeffs();
    [a.hypot(b), c.hypot(d)]
}

/// Reports whether two scales are alike within [`EPSILON`].
fn alike([kx, ky]: [f64; 2]) -> bool {
    (kx - ky).abs() <= f64::from(EPSILON) * kx.max(ky)
}

/// Returns `stroke` with its width and dashes multiplied by `scale`.
fn scaled(stroke: &Stroke, scale: f64) -> Stroke {
    let mut scaled = stroke.clone();
    scaled.width *= scale;
    scaled.dash_offset *= scale;
    for dash in scaled.dash_pattern.iter_mut() {
        *dash *= scale;
    }
    scaled
}

/// Returns the mask style of `stroke` with its lengths in texels at `density`, or `None` when
/// its width is not positive or a length is not finite.
fn stroke_style(stroke: &Stroke, density: f32) -> Option<MaskStyle> {
    let finite = [stroke.width, stroke.miter_limit, stroke.dash_offset]
        .into_iter()
        .chain(stroke.dash_pattern.iter().copied())
        .all(f64::is_finite);
    if !finite || stroke.width <= 0.0 {
        return None;
    }

    let texels = |length: f64| ((length * f64::from(density)) as f32).to_bits();
    let cap = |cap: Cap| match cap {
        Cap::Butt => StrokeCap::Butt,
        Cap::Square => StrokeCap::Square,
        Cap::Round => StrokeCap::Round,
    };
    Some(MaskStyle::Stroke {
        width: texels(stroke.width),
        join: match stroke.join {
            Join::Bevel => StrokeJoin::Bevel,
            Join::Miter => StrokeJoin::Miter,
            Join::Round => StrokeJoin::Round,
        },
        miter_limit: (stroke.miter_limit as f32).to_bits(),
        caps: [cap(stroke.start_cap), cap(stroke.end_cap)],
        dashes: stroke
            .dash_pattern
            .iter()
            .map(|dash| texels(*dash))
            .collect(),
        dash_offset: texels(stroke.dash_offset),
    })
}
