//! Which of the two text routes a run takes.

use iceblink::geom::Matrix4;

/// The largest em size on the atlas route, in logical pixels.
///
/// It is multiplied by the output scale, at least 1, and capped by [`ATLAS_MAX_SIZE`].
pub const ATLAS_MAX_LOGICAL_SIZE: f32 = 96.0;

/// The largest em size on the atlas route, in device pixels.
pub const ATLAS_MAX_SIZE: f32 = 256.0;

/// The most a moving upright scale may stretch or shrink cached tiles on each axis.
pub const ATLAS_MAX_STRETCH: f32 = 1.25;

/// The largest relative difference between the two scale factors of a uniform scale.
const UNIFORM: f32 = 1.0e-6;

/// How a run's glyphs are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Route {
    /// Tiles rasterised at the device size and placed on whole device pixels.
    Exact(Grid),
    /// Tiles rasterised at the em size with phase zero, resampled through the world matrix.
    Resampled {
        /// The horizontal translation of the world matrix.
        tx: f32,
        /// The vertical translation of the world matrix.
        ty: f32,
    },
    /// Outlines filled as paths.
    Outline,
}

/// The upright map from local space onto the device pixel grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grid {
    /// The horizontal scale.
    pub(crate) sx: f32,
    /// The vertical scale.
    pub(crate) sy: f32,
    /// The horizontal translation.
    pub(crate) tx: f32,
    /// The vertical translation.
    pub(crate) ty: f32,
}

impl Grid {
    /// Returns the device position of the local point `[x, y]`.
    pub(crate) fn device(self, x: f32, y: f32) -> [f32; 2] {
        [self.sx * x + self.tx, self.sy * y + self.ty]
    }

    /// Returns the local rectangle that covers the device rectangle `[x, y, width, height]`.
    pub(crate) fn local(self, rect: [f32; 4]) -> [f32; 4] {
        [
            (rect[0] - self.tx) / self.sx,
            (rect[1] - self.ty) / self.sy,
            rect[2] / self.sx,
            rect[3] / self.sy,
        ]
    }
}

/// What a run's paint and size allow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RunSurface {
    /// Whether the paint is a solid colour.
    pub(crate) solid: bool,
    /// The em size, in local units.
    pub(crate) size: f32,
    /// The output's scale factor.
    pub(crate) output_scale: f32,
}

/// How a world matrix lets glyph tiles be placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Placing {
    /// On whole device pixels, under a uniform positive scale.
    Grid(Grid),
    /// Resampled under a moving scale whose factors lie within [`ATLAS_MAX_STRETCH`] of one.
    Stretch {
        /// The horizontal translation of the world matrix.
        tx: f32,
        /// The vertical translation of the world matrix.
        ty: f32,
    },
    /// Resampled under any other matrix.
    Free,
}

/// Returns how tiles are placed under `world`.
///
/// The placing depends on the matrix:
///
/// - If it is not two-dimensional, or not upright with positive factors, tiles are placed freely.
/// - If `moving` is set, a factor differs from one and both factors lie within
///   [`ATLAS_MAX_STRETCH`] of one, tiles are stretched.
/// - If both factors are equal, tiles are placed on whole device pixels.
/// - Otherwise tiles are placed freely.
pub(crate) fn placing_of(world: &Matrix4, moving: bool) -> Placing {
    let Some(affine) = world.to_affine2() else {
        return Placing::Free;
    };
    let upright = affine.b == 0.0 && affine.c == 0.0 && affine.a > 0.0 && affine.d > 0.0;
    if !upright {
        return Placing::Free;
    }

    let near = |factor: f32| (1.0 / ATLAS_MAX_STRETCH..=ATLAS_MAX_STRETCH).contains(&factor);
    let stretched = affine.a != 1.0 || affine.d != 1.0;
    if moving && stretched && near(affine.a) && near(affine.d) {
        return Placing::Stretch {
            tx: affine.tx,
            ty: affine.ty,
        };
    }

    if (affine.a - affine.d).abs() <= UNIFORM * affine.a.max(affine.d) {
        return Placing::Grid(Grid {
            sx: affine.a,
            sy: affine.d,
            tx: affine.tx,
            ty: affine.ty,
        });
    }
    Placing::Free
}

/// Reports whether the glyph tiles of a run lowered under `world` sit on whole device pixels,
/// which they do under a uniform positive scale unless `moving` lets them stretch.
pub fn tiles_on_grid(world: &Matrix4, moving: bool) -> bool {
    matches!(placing_of(world, moving), Placing::Grid(_))
}

/// Reports whether glyph tiles under `world` are stretched while it moves, so that a run lowered
/// under it changes when it starts or stops moving.
pub fn stretches(world: &Matrix4) -> bool {
    matches!(placing_of(world, true), Placing::Stretch { .. })
}

/// Returns the route of a run's plain glyphs on `surface`, placed as `placing` allows.
///
/// The atlas route needs all of these:
///
/// 1. The paint is solid.
/// 2. The tiles are placed on the grid or stretched.
/// 3. The size the tiles are rasterised at is at most the atlas limit for the output scale.
///
/// Every other run is drawn from outlines.
pub(crate) fn route_of(surface: RunSurface, placing: Placing) -> Route {
    if !surface.solid {
        return Route::Outline;
    }

    let limit = atlas_max_size(surface.output_scale);
    match placing {
        Placing::Grid(grid) if surface.size * grid.sx.max(grid.sy) <= limit => Route::Exact(grid),
        Placing::Stretch { tx, ty } if surface.size <= limit => Route::Resampled { tx, ty },
        _ => Route::Outline,
    }
}

/// Returns the largest device size on the atlas route at `scale` device pixels per logical
/// pixel: 96 at 1, 192 at 2 and 256 from 2.67 upwards.
pub(crate) fn atlas_max_size(scale: f32) -> f32 {
    (ATLAS_MAX_LOGICAL_SIZE * scale.max(1.0)).min(ATLAS_MAX_SIZE)
}
