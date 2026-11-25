//! Resolving a clip entry into its GPU record against the current world matrices.
//!
//! The links are sorted into three forms, fastest first:
//!
//! 1. A hard edge becomes part of the device box. A link in a space that translates is moved
//!    onto the device, and a zero-radius link in a space that keeps the axes or turns them by
//!    quarter turns is a hard edge.
//! 2. Up to two rounded links in spaces that translate become device-space rounded tests.
//! 3. Every other link is a general link, evaluated in the shader in its own space with the
//!    space's current matrix.
//!
//! The innermost rounded link's space is kept when it translates: its links are stored in its own
//! coordinates and tested at the device point moved back by the current translation, so moving
//! that space changes no clip record. Forms 1 and 2 depend on the matrix when the clip was
//! resolved, and the clip is resolved again when such a space changes.

use iceblink::{AtlasTile, ClipEntry, CornerShapes, FrameClipShape, MaskMode, Matrix4};
use smallvec::SmallVec;

use crate::bind::tables::records::{GpuClip, GpuClipLink, GpuRounded, NO_SPACE, UNBOUNDED, link};
use crate::bind::tables::spaces::{axis_aligned, translation};

/// How a resolved clip depends on a space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Membership {
    /// Links in the space were moved onto the device with its matrix.
    Uses(u32),
    /// The space is kept, which holds while it translates.
    Keeps(u32),
}

/// A clip entry resolved into its record and general links.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Resolved {
    pub(crate) clip: GpuClip,
    /// The general links.
    pub(crate) links: SmallVec<[GpuClipLink; 2]>,
    /// The mask links among the general links.
    pub(crate) masks: SmallVec<[MaskLink; 1]>,
    /// The spaces the record depends on.
    pub(crate) memberships: SmallVec<[Membership; 2]>,
}

/// A mask link whose cell the mask store places, by its index among the general links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MaskLink {
    pub(crate) link: usize,
    /// The atlas page of the mask's tile.
    pub(crate) page: u32,
    /// The tile's texels in the page, as origin and extent.
    pub(crate) rect: [u32; 4],
}

/// Resolves `entry` against `spaces`.
///
/// A link naming a space past the table is resolved against the identity.
pub(crate) fn resolve(entry: &ClipEntry, spaces: &[Matrix4]) -> Resolved {
    let mut sorting = Sorting::new(entry, spaces);
    for link in &entry.links {
        match &link.shape {
            FrameClipShape::RoundedRect {
                rect,
                radii,
                shapes,
            } => sorting.rounded(link.space, *rect, *radii, *shapes),
            FrameClipShape::Mask { rect, tile, mode } => {
                sorting.mask(link.space, *rect, tile, *mode);
            }
        }
    }
    sorting.finish()
}

/// A clip entry's links, sorted into the three forms one at a time.
struct Sorting<'a> {
    spaces: &'a [Matrix4],
    /// The space of the innermost rounded link, if it translates.
    kept: Option<u32>,
    resolved: Resolved,
    aabb: [f32; 4],
    kept_aabb: [f32; 4],
    /// The rounded tests, each marked when it is in the kept space.
    tests: SmallVec<[(GpuRounded, bool); 2]>,
}

impl<'a> Sorting<'a> {
    /// Starts sorting the links of `entry` against `spaces`.
    fn new(entry: &ClipEntry, spaces: &'a [Matrix4]) -> Self {
        let mut sorting = Self {
            spaces,
            kept: None,
            resolved: Resolved::default(),
            aabb: UNBOUNDED,
            kept_aabb: UNBOUNDED,
            tests: SmallVec::new(),
        };
        sorting.kept = entry
            .links
            .iter()
            .rev()
            .find(|link| matches!(link.shape, FrameClipShape::RoundedRect { .. }))
            .map(|link| link.space)
            .filter(|&space| translation(sorting.matrix(space)).is_some());
        if let Some(space) = sorting.kept {
            sorting.resolved.memberships.push(Membership::Keeps(space));
        }
        sorting
    }

    /// Returns the matrix of space `slot`, or the identity for a slot past the table.
    fn matrix(&self, slot: u32) -> &'a Matrix4 {
        self.spaces.get(slot as usize).unwrap_or(&Matrix4::IDENTITY)
    }

    /// Sorts one rounded link into the fastest form it takes.
    fn rounded(&mut self, space: u32, rect: [f32; 4], radii: [f32; 8], shapes: CornerShapes) {
        let square = radii.iter().all(|radius| *radius == 0.0);
        let rounded = GpuRounded {
            rect,
            radii,
            shapes,
        };
        if Some(space) == self.kept {
            if square {
                self.kept_aabb = intersect(self.kept_aabb, rect);
                return;
            }
            if self.tests.len() < 2 {
                self.tests.push((rounded, true));
                return;
            }
        } else if let Some([x, y]) = translation(self.matrix(space)) {
            let moved = [rect[0] + x, rect[1] + y, rect[2], rect[3]];
            if square {
                self.aabb = intersect(self.aabb, moved);
                self.uses(space);
                return;
            }
            if self.tests.len() < 2 {
                self.tests.push((
                    GpuRounded {
                        rect: moved,
                        ..rounded
                    },
                    false,
                ));
                self.uses(space);
                return;
            }
        } else if square && let Some(map) = axis_aligned(self.matrix(space)) {
            self.aabb = intersect(self.aabb, mapped(rect, map));
            self.uses(space);
            return;
        }
        self.resolved.links.push(GpuClipLink {
            kind: link::ROUNDED,
            space,
            rect,
            radii,
            extra: shapes,
            ..GpuClipLink::default()
        });
    }

    /// Adds one mask link as a general link whose cell is still to be placed.
    fn mask(&mut self, space: u32, rect: [f32; 4], tile: &AtlasTile, mode: MaskMode) {
        self.resolved.masks.push(MaskLink {
            link: self.resolved.links.len(),
            page: tile.page,
            rect: tile.rect,
        });
        self.resolved.links.push(GpuClipLink {
            kind: link::ALPHA_MASK,
            space,
            mode: u32::from(mode == MaskMode::LUMINANCE),
            rect,
            ..GpuClipLink::default()
        });
    }

    /// Notes that the clip moved links in `space` onto the device.
    fn uses(&mut self, space: u32) {
        let membership = Membership::Uses(space);
        if !self.resolved.memberships.contains(&membership) {
            self.resolved.memberships.push(membership);
        }
    }

    /// Returns the resolved clip.
    fn finish(self) -> Resolved {
        let Self {
            kept,
            mut resolved,
            aabb,
            kept_aabb,
            tests,
            ..
        } = self;
        let test = |index: usize| tests.get(index).map(|(test, _)| *test).unwrap_or_default();
        resolved.clip = GpuClip {
            aabb,
            first: test(0),
            second: test(1),
            kept_aabb,
            count: tests.len() as u32,
            space: kept.unwrap_or(NO_SPACE),
            kept_tests: tests
                .iter()
                .enumerate()
                .filter(|(_, (_, kept))| *kept)
                .map(|(index, _)| 1 << index)
                .sum(),
            links: 0,
        };
        resolved
    }
}

/// Returns `rect` mapped through the plane map `[a, b, c, d, tx, ty]`, as origin and extent.
fn mapped(rect: [f32; 4], [a, b, c, d, tx, ty]: [f32; 6]) -> [f32; 4] {
    let point = |x: f32, y: f32| [a * x + c * y + tx, b * x + d * y + ty];
    let [x0, y0] = point(rect[0], rect[1]);
    let [x1, y1] = point(rect[0] + rect[2], rect[1] + rect[3]);
    [x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs()]
}

/// Returns the intersection of two boxes, as origin and extent.
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
