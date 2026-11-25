//! The keys a run's glyphs are rasterised under, and where their bitmaps are drawn.

use iceblink::{
    Glyph, GlyphFlags, GlyphKey, GlyphPlacement, GlyphRun, PaintKind, PenPosition, RasterStyle,
};

use crate::content::glyph::route::{Grid, Placing, Route, RunSurface, placing_of, route_of};
use crate::content::input::GlyphRunInput;
use crate::content::target::Target;

/// The route and key template shared by every glyph of one run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RunKeys {
    /// The route of the run's plain glyphs.
    pub(crate) route: Route,
    /// How the world matrix lets tiles be placed.
    placing: Placing,
    /// The style of the run's plain glyphs on the atlas route.
    style: RasterStyle,
    /// Whether colour glyphs draw nothing.
    pub(crate) silhouette: bool,
    /// The key of glyph zero at phase zero, at the em size.
    base: GlyphKey,
    /// The chunk offset.
    offset: [f32; 2],
}

/// Where one glyph's bitmap is drawn from.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Spot {
    /// On whole device pixels under `grid`, from the split of the device position.
    Exact {
        grid: Grid,
        pen: PenPosition,
        baseline: f32,
    },
    /// At a local position with phase zero, resampled through the world matrix.
    Resampled {
        /// The local x of the pixel the bitmap's left edge is measured from.
        x: f32,
        /// The local y of the baseline row.
        y: f32,
    },
}

impl Spot {
    /// Returns the local rectangle a bitmap of `size` with `placement` covers.
    pub(crate) fn bounds(self, placement: GlyphPlacement, size: [u32; 2]) -> [f32; 4] {
        let [width, height] = [size[0] as f32, size[1] as f32];
        match self {
            Self::Exact {
                grid,
                pen,
                baseline,
            } => {
                let [x, y] = pen.bitmap_origin(baseline, placement);
                grid.local([x, y, width, height])
            }
            Self::Resampled { x, y } => [
                x + placement.left as f32,
                y - placement.top as f32,
                width,
                height,
            ],
        }
    }
}

impl RunKeys {
    /// Returns the keys of `input` drawn at `target`, with `instance` as the interned number of
    /// its variation coordinates.
    pub(crate) fn of(input: &GlyphRunInput<'_>, target: &Target, instance: u32) -> Self {
        let run: &GlyphRun = input.run;
        let placing = placing_of(&target.world, target.moving);
        let route = route_of(
            RunSurface {
                solid: matches!(run.paint.kind, PaintKind::SOLID | PaintKind::INLINE),
                size: run.size,
                output_scale: input.output_scale,
            },
            placing,
        );

        let silhouette = run.flags.contains(GlyphFlags::SILHOUETTE);
        let subpixel = matches!(route, Route::Exact(_))
            && input.subpixel
            && run.flags.contains(GlyphFlags::SUBPIXEL)
            && !silhouette;

        Self {
            route,
            placing,
            style: if subpixel {
                RasterStyle::Subpixel
            } else {
                RasterStyle::Mono
            },
            silhouette,
            base: GlyphKey {
                font: run.font,
                glyph: 0,
                size_bits: run.size.to_bits(),
                instance,
                phase: 0,
                style: RasterStyle::Mono,
                hint: run.flags.contains(GlyphFlags::HINT),
                palette: run.palette,
                embolden_bits: run.embolden.to_bits(),
                skew_bits: run.skew.to_bits(),
            },
            offset: target.offset,
        }
    }

    /// Returns the key and spot of a plain glyph drawn from a tile.
    ///
    /// A run on the outline route draws a glyph without an outline from a mono tile, placed as
    /// the world matrix allows.
    pub(crate) fn plain(&self, glyph: &Glyph) -> (GlyphKey, Spot) {
        match self.route {
            Route::Exact(grid) => self.exact(grid, glyph, self.style),
            Route::Resampled { tx, ty } => self.resampled(glyph, RasterStyle::Mono, tx, ty),
            Route::Outline => self.placed(glyph, RasterStyle::Mono),
        }
    }

    /// Returns the key and spot of a colour glyph, which is drawn from a tile on every route.
    pub(crate) fn color(&self, glyph: &Glyph) -> (GlyphKey, Spot) {
        match self.route {
            Route::Exact(grid) => self.exact(grid, glyph, RasterStyle::Color),
            Route::Resampled { tx, ty } => self.resampled(glyph, RasterStyle::Color, tx, ty),
            Route::Outline => self.placed(glyph, RasterStyle::Color),
        }
    }

    /// Returns the local origin of `glyph`, with the chunk offset applied.
    pub(crate) fn origin(&self, glyph: &Glyph) -> [f32; 2] {
        [glyph.x + self.offset[0], glyph.y + self.offset[1]]
    }

    /// Returns the outline key template of the run.
    pub(crate) fn base(&self) -> GlyphKey {
        self.base
    }

    /// Returns the key and spot of a glyph off the atlas route, on whole device pixels where the
    /// world matrix allows it.
    fn placed(&self, glyph: &Glyph, style: RasterStyle) -> (GlyphKey, Spot) {
        match self.placing {
            Placing::Grid(grid) => self.exact(grid, glyph, style),
            Placing::Stretch { tx, ty } => self.resampled(glyph, style, tx, ty),
            Placing::Free => self.resampled(glyph, style, 0.0, 0.0),
        }
    }

    fn exact(&self, grid: Grid, glyph: &Glyph, style: RasterStyle) -> (GlyphKey, Spot) {
        let [x, y] = self.origin(glyph);
        let [device_x, device_y] = grid.device(x, y);
        let pen = PenPosition::of(device_x);
        let size = f32::from_bits(self.base.size_bits) * grid.sx.max(grid.sy);
        let key = GlyphKey {
            glyph: glyph.id,
            size_bits: size.to_bits(),
            phase: pen.phase(),
            style,
            ..self.base
        };

        (
            key,
            Spot::Exact {
                grid,
                pen,
                baseline: device_y,
            },
        )
    }

    fn resampled(&self, glyph: &Glyph, style: RasterStyle, tx: f32, ty: f32) -> (GlyphKey, Spot) {
        let [x, y] = self.origin(glyph);
        let key = GlyphKey {
            glyph: glyph.id,
            style,
            ..self.base
        };
        (
            key,
            Spot::Resampled {
                x: (x + tx).round() - tx,
                y: (y + ty).round() - ty,
            },
        )
    }
}
