//! Clip chains resolved against the current world matrices, and their coverage at a device point.
//!
//! A hard edge becomes part of a device box: a square link in a space that translates, keeps the
//! axes or turns them by quarter turns. A rounded link in a space that translates becomes a
//! rounded test on the device. Every other link is evaluated in its own space at the plane point
//! that maps to the pixel.

use iceblink::{AtlasTile, ClipEntry, FrameClipShape, MaskMode, Matrix4, PageFormat};
use smallvec::SmallVec;

use crate::math::Floor as _;
use crate::math::sdf::{outside_bounds, rect_coverage};
use crate::math::vec::mix4;
use crate::pages::{Page, Pages};
use crate::tables::space::Space;

/// A box that holds every device point.
const UNBOUNDED: [f32; 4] = [-1.0e9, -1.0e9, 2.0e9, 2.0e9];

/// A rounded rectangle test.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rounded {
    rect: [f32; 4],
    radii: [f32; 8],
    shapes: [f32; 4],
}

impl Rounded {
    /// Returns the coverage of `point`, which is 0 outside the rectangle grown by half a unit.
    fn coverage(&self, point: [f32; 2]) -> f32 {
        // The distance underestimates far from a thin elliptical corner.
        let [x, y, width, height] = self.rect;
        if outside_bounds(point, [x - 0.5, y - 0.5, width + 1.0, height + 1.0]) {
            return 0.0;
        }
        rect_coverage(point, self.rect, &self.radii, &self.shapes)
    }
}

/// A link evaluated in its own space.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Link {
    Rounded {
        space: u32,
        test: Rounded,
    },
    Mask {
        space: u32,
        rect: [f32; 4],
        tile: AtlasTile,
        mode: MaskMode,
    },
}

/// A clip chain resolved for one frame's world matrices.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Clip {
    aabb: [f32; 4],
    tests: SmallVec<[Rounded; 2]>,
    links: SmallVec<[Link; 2]>,
}

impl Default for Clip {
    fn default() -> Self {
        Self {
            aabb: UNBOUNDED,
            tests: SmallVec::new(),
            links: SmallVec::new(),
        }
    }
}

impl Clip {
    /// Resolves `entry` against `matrices`.
    ///
    /// A link whose space slot lies past the table is measured in device space.
    pub(crate) fn resolve(entry: &ClipEntry, matrices: &[Matrix4]) -> Self {
        let matrix = |slot: u32| matrices.get(slot as usize).unwrap_or(&Matrix4::IDENTITY);
        let mut clip = Self::default();
        for link in &entry.links {
            match &link.shape {
                FrameClipShape::RoundedRect {
                    rect,
                    radii,
                    shapes,
                } => {
                    let test = Rounded {
                        rect: *rect,
                        radii: *radii,
                        shapes: *shapes,
                    };
                    clip.add_rounded(test, link.space, matrix(link.space));
                }
                FrameClipShape::Mask { rect, tile, mode } => clip.links.push(Link::Mask {
                    space: link.space,
                    rect: *rect,
                    tile: *tile,
                    mode: *mode,
                }),
            }
        }
        clip
    }

    // Adds a rounded link in `space`, whose world matrix is `matrix`, as part of the device box, as
    // a rounded test on the device, or as a link evaluated in its own space.
    fn add_rounded(&mut self, test: Rounded, space: u32, matrix: &Matrix4) {
        let square = test.radii.iter().all(|radius| *radius == 0.0);
        let rect = test.rect;
        if let Some((x, y)) = matrix.plane_translation() {
            let moved = [rect[0] + x, rect[1] + y, rect[2], rect[3]];
            if square {
                self.aabb = intersect(self.aabb, moved);
            } else {
                self.tests.push(Rounded {
                    rect: moved,
                    ..test
                });
            }
            return;
        }
        if square && let Some(map) = axis_aligned(matrix) {
            self.aabb = intersect(self.aabb, mapped(rect, map));
            return;
        }
        self.links.push(Link::Rounded { space, test });
    }

    /// Returns the device box of a clip that is a box alone, as `[x, y, width, height]`, edges
    /// included.
    pub(crate) fn box_only(&self) -> Option<[f32; 4]> {
        (self.tests.is_empty() && self.links.is_empty()).then_some(self.aabb)
    }

    /// Returns the coverage of the device point `point`, evaluating each link in its own space.
    pub(crate) fn coverage(&self, point: [f32; 2], spaces: &[Space], pages: &Pages) -> f32 {
        if outside_bounds(point, self.aabb) {
            return 0.0;
        }
        let mut coverage = 1.0;
        for test in &self.tests {
            coverage *= test.coverage(point);
        }
        if coverage <= 0.0 {
            return 0.0;
        }
        for link in &self.links {
            coverage *= link_coverage(link, point, spaces, pages);
            if coverage <= 0.0 {
                return 0.0;
            }
        }
        coverage
    }
}

fn link_coverage(link: &Link, device: [f32; 2], spaces: &[Space], pages: &Pages) -> f32 {
    let plane = |space: u32| {
        spaces
            .get(space as usize)
            .map_or(device, |space| space.plane(device))
    };
    match *link {
        Link::Rounded { space, test } => test.coverage(plane(space)),
        Link::Mask {
            space,
            rect,
            tile,
            mode,
        } => {
            let point = plane(space);
            if outside_bounds(point, rect) {
                return 0.0;
            }
            let Some(page) = pages.get(tile.page) else {
                return 0.0;
            };
            let cell = tile.rect.map(|value| value as f32);
            let uv = [
                (point[0] - rect[0]) / rect[2].max(1.0e-6),
                (point[1] - rect[1]) / rect[3].max(1.0e-6),
            ];
            let texel = [cell[0] + uv[0] * cell[2], cell[1] + uv[1] * cell[3]];
            let color = mask_texel(page, cell, texel);
            if page.format == PageFormat::A8 {
                return color[0];
            }
            if mode == MaskMode::LUMINANCE {
                // The texels are premultiplied, so this is the straight luminance times alpha.
                return (color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722).clamp(0.0, 1.0);
            }
            color[3]
        }
    }
}

// The page sampled bilinearly at a texel position, never reading outside `cell`.
fn mask_texel(page: &Page, cell: [f32; 4], texel: [f32; 2]) -> [f32; 4] {
    let low = [cell[0] + 0.5, cell[1] + 0.5];
    let high = [
        (cell[0] + cell[2] - 0.5).max(low[0]),
        (cell[1] + cell[3] - 0.5).max(low[1]),
    ];
    let at = [
        texel[0].clamp(low[0], high[0]) - 0.5,
        texel[1].clamp(low[1], high[1]) - 0.5,
    ];
    let base = [at[0].floored(), at[1].floored()];
    let weight = [at[0] - base[0], at[1] - base[1]];
    let first = [cell[0] as i32, cell[1] as i32];
    let last = [
        (cell[0] + cell[2]) as i32 - 1,
        (cell[1] + cell[3]) as i32 - 1,
    ];
    let a = [base[0] as i32, base[1] as i32];
    let b = [(a[0] + 1).min(last[0]), (a[1] + 1).min(last[1])];
    let c00 = page.load(
        0,
        a[0].clamp(first[0], last[0]),
        a[1].clamp(first[1], last[1]),
    );
    let c10 = page.load(0, b[0], a[1]);
    let c01 = page.load(0, a[0], b[1]);
    let c11 = page.load(0, b[0], b[1]);
    mix4(
        mix4(c00, c10, weight[0]),
        mix4(c01, c11, weight[0]),
        weight[1],
    )
}

/// The plane map `[a, b, c, d, tx, ty]` of a matrix that keeps the axes or turns them by quarter
/// turns, without projecting.
fn axis_aligned(matrix: &Matrix4) -> Option<[f32; 6]> {
    let c = &matrix.columns;
    let w = c[3][3];
    if c[0][3] != 0.0 || c[1][3] != 0.0 || w <= 0.0 {
        return None;
    }
    let [a, b, cx, d] = [c[0][0], c[0][1], c[1][0], c[1][1]];
    let scales = b == 0.0 && cx == 0.0 && a != 0.0 && d != 0.0;
    let turns = a == 0.0 && d == 0.0 && b != 0.0 && cx != 0.0;
    (scales || turns).then(|| [a / w, b / w, cx / w, d / w, c[3][0] / w, c[3][1] / w])
}

fn mapped(rect: [f32; 4], [a, b, c, d, tx, ty]: [f32; 6]) -> [f32; 4] {
    let point = |x: f32, y: f32| [a * x + c * y + tx, b * x + d * y + ty];
    let [x0, y0] = point(rect[0], rect[1]);
    let [x1, y1] = point(rect[0] + rect[2], rect[1] + rect[3]);
    [x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs()]
}

// An empty intersection is a box of zero size, which rejects every point but its corner.
fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let left = a[0].max(b[0]);
    let top = a[1].max(b[1]);
    let right = (a[0] + a[2]).min(b[0] + b[2]);
    let bottom = (a[1] + a[3]).min(b[1] + b[3]);
    if right < left || bottom < top {
        return [-1.0e9, -1.0e9, -1.0, -1.0];
    }
    [left, top, right - left, bottom - top]
}
