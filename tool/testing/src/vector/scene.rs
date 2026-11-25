//! Hand-built vector frames.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::kurbo::{self, Affine, BezPath, Shape as _};
use iceblink::{
    ClipEntry, ClipLinkEntry, Color, DeviceRect, FRAME_NO_CLIP, FillRule, FramePaint, FramePath,
    Matrix4, PaintKind, PaintRef, PassKey, PathFill, PathStroke, PlannedPass, PlannedPath,
    VectorPlan,
};
use iceblink_wgpu::vector::{ResourcedPlan, VectorFrame};

use crate::gpu::TestDevice;

/// The paths, tables and vector plan of one frame on a square output.
pub struct Scene {
    /// The width and height of the output, in pixels.
    pub side: i32,
    /// The frame paths.
    pub paths: Vec<FramePath>,
    /// The frame paint table.
    pub paints: Vec<FramePaint>,
    /// The frame clip table.
    pub clips: Vec<ClipEntry>,
    /// The frame space table.
    pub spaces: Vec<Matrix4>,
    /// The vector plan.
    pub plan: VectorPlan,
}

impl Scene {
    /// Returns an empty frame on an output of `side` square, whose space slot 0 is the identity.
    pub fn new(side: i32) -> Self {
        Self {
            side,
            paths: Vec::new(),
            paints: Vec::new(),
            clips: Vec::new(),
            spaces: vec![Matrix4::IDENTITY],
            plan: VectorPlan::default(),
        }
    }

    /// Adds a paint slot holding `paint`.
    pub fn paint(&mut self, paint: FramePaint) -> PaintRef {
        let kind = match paint {
            FramePaint::None => PaintKind::NONE,
            FramePaint::Solid(_) => PaintKind::SOLID,
            FramePaint::Gradient(_) => PaintKind::GRADIENT,
            FramePaint::Sides(_) => PaintKind::SIDES,
        };
        self.paints.push(paint);
        PaintRef {
            kind,
            _pad: [0; 3],
            index: self.paints.len() as u32 - 1,
        }
    }

    /// Adds a solid paint of the straight sRGB bytes `rgba`.
    pub fn solid(&mut self, rgba: [u8; 4]) -> PaintRef {
        let [r, g, b, a] = rgba.map(|channel| f32::from(channel) / 255.0);
        self.paint(FramePaint::Solid(Color::from_straight(r, g, b, a)))
    }

    /// Adds `path` filled with `paint` by the non-zero rule, in device space.
    pub fn fill(&mut self, path: Arc<BezPath>, paint: PaintRef) -> u32 {
        self.add(FramePath {
            path,
            transform: Affine::IDENTITY,
            brush: Affine::IDENTITY,
            fill: Some(PathFill {
                paint,
                rule: FillRule::NON_ZERO,
            }),
            stroke: None,
            clips: Arc::new([]),
            ink: Rect::default(),
        })
    }

    /// Adds `path` stroked with `paint` and `style`, in device space.
    pub fn stroke(&mut self, path: Arc<BezPath>, paint: PaintRef, style: kurbo::Stroke) -> u32 {
        self.add(FramePath {
            path,
            transform: Affine::IDENTITY,
            brush: Affine::IDENTITY,
            fill: None,
            stroke: Some(PathStroke { paint, style }),
            clips: Arc::new([]),
            ink: Rect::default(),
        })
    }

    /// Adds `path` with its ink computed, and returns its index.
    pub fn add(&mut self, mut path: FramePath) -> u32 {
        path.ink = ink(&path);
        self.paths.push(path);
        self.paths.len() as u32 - 1
    }

    /// Places path `path` under `transform`, and returns it.
    pub fn transformed(&mut self, path: u32, transform: Affine) -> u32 {
        let placed = &mut self.paths[path as usize];
        placed.transform = transform;
        placed.ink = ink(placed);
        path
    }

    /// Adds a space slot holding `matrix`.
    pub fn space(&mut self, matrix: Matrix4) -> u32 {
        self.spaces.push(matrix);
        self.spaces.len() as u32 - 1
    }

    /// Adds a clip slot holding a chain of `links`, outermost first.
    pub fn clip(&mut self, links: Vec<ClipLinkEntry>) -> u32 {
        self.clips.push(ClipEntry {
            links: links.into(),
        });
        self.clips.len() as u32 - 1
    }

    /// Plans one pass of `paths`, each drawn with no residual.
    pub fn pass(&mut self, paths: &[u32]) -> &mut Self {
        let planned: Vec<(u32, u32)> = paths.iter().map(|&path| (path, FRAME_NO_CLIP)).collect();
        self.pass_of(&planned, false, None)
    }

    /// Plans one pass of `paths`, as path and residual slot pairs.
    ///
    /// The region is the union of the inks aligned outwards to 16 pixels and cut to the output,
    /// and then to `cut` when one is given, which clamps the pass.
    pub fn pass_of(
        &mut self,
        paths: &[(u32, u32)],
        instanced: bool,
        cut: Option<DeviceRect>,
    ) -> &mut Self {
        let union = paths
            .iter()
            .map(|&(path, _)| self.paths[path as usize].ink)
            .reduce(union)
            .unwrap_or_default();
        let output = Rect::new(Point::new(0, 0), Size::new(self.side, self.side));
        let aligned = align_out(union);
        let mut region = intersection(aligned, output);
        let overhangs = region != aligned;
        if let Some(cut) = cut {
            region = intersection(region, cut);
        }
        let clamped = cut.is_some_and(|_| intersection(union, region) != union);

        let first = self.plan.items.len() as u32;
        for &(path, residual) in paths {
            let ink = self.paths[path as usize].ink;
            self.plan.items.push(PlannedPath {
                path,
                residual,
                clip: FRAME_NO_CLIP,
                ink: Rect::new(
                    Point::new(
                        ink.origin.x - region.origin.x,
                        ink.origin.y - region.origin.y,
                    ),
                    ink.size,
                ),
            });
        }
        self.plan.passes.push(PlannedPass {
            items: first..self.plan.items.len() as u32,
            region,
            clip: FRAME_NO_CLIP,
            instanced,
            clamped,
            overhangs,
            key: PassKey::NONE,
        });
        self
    }

    /// Returns the work of `round` of `plan` over this frame's tables.
    pub fn frame<'frame>(
        &'frame self,
        plan: &'frame ResourcedPlan,
        round: usize,
        gpu: &'frame TestDevice,
    ) -> VectorFrame<'frame> {
        VectorFrame {
            plan,
            paths: &self.paths,
            clips: &self.clips,
            paints: &self.paints,
            spaces: &self.spaces,
            round,
            device: &gpu.device,
            queue: &gpu.queue,
        }
    }
}

/// Returns a rectangular path.
pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Arc<BezPath> {
    Arc::new(kurbo::Rect::new(x, y, x + width, y + height).to_path(0.01))
}

/// Returns a circular path.
pub fn circle(x: f64, y: f64, radius: f64) -> Arc<BezPath> {
    Arc::new(kurbo::Circle::new((x, y), radius).to_path(0.01))
}

/// Returns the device pixels `path` can change: its control box outset by the stroke's reach,
/// mapped through its transform and grown to whole pixels.
pub fn ink(path: &FramePath) -> DeviceRect {
    let reach = path.stroke.as_ref().map_or(0.0, |stroke| {
        let style = &stroke.style;
        let join = if style.join == kurbo::Join::Miter {
            style.miter_limit.max(1.0)
        } else {
            1.0
        };
        let square = [style.start_cap, style.end_cap].contains(&kurbo::Cap::Square);
        let cap = if square {
            std::f64::consts::SQRT_2
        } else {
            1.0
        };
        style.width / 2.0 * join.max(cap)
    });
    let local = path.path.control_box().inflate(reach, reach);
    let device = path.transform.transform_rect_bbox(local);
    Rect::from_corners(
        Point::new(device.x0.floor() as i32, device.y0.floor() as i32),
        Point::new(device.x1.ceil() as i32, device.y1.ceil() as i32),
    )
}

/// Returns the smallest rectangle on the 16-pixel grid that holds `rect`.
fn align_out(rect: DeviceRect) -> DeviceRect {
    Rect::from_corners(
        Point::new(
            rect.left().div_euclid(16) * 16,
            rect.top().div_euclid(16) * 16,
        ),
        Point::new(
            (rect.right() + 15).div_euclid(16) * 16,
            (rect.bottom() + 15).div_euclid(16) * 16,
        ),
    )
}

/// Returns the smallest rectangle that holds both.
fn union(one: DeviceRect, two: DeviceRect) -> DeviceRect {
    Rect::from_corners(
        Point::new(one.left().min(two.left()), one.top().min(two.top())),
        Point::new(one.right().max(two.right()), one.bottom().max(two.bottom())),
    )
}

/// Returns the pixels both hold, or an empty rectangle.
fn intersection(one: DeviceRect, two: DeviceRect) -> DeviceRect {
    let left = one.left().max(two.left());
    let top = one.top().max(two.top());
    let right = one.right().min(two.right()).max(left);
    let bottom = one.bottom().min(two.bottom()).max(top);
    Rect::from_corners(Point::new(left, top), Point::new(right, bottom))
}
