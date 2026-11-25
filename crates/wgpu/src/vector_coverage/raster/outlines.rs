//! Flattened outlines kept across frames.

use std::sync::Arc;

use kurbo::Affine;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::vector_coverage::geometry::{self, Segment};

/// The path and outline kind a cache entry is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct OutlineKey {
    /// The address of the path's allocation.
    path: usize,
    /// Whether the outline is the stroke's.
    stroked: bool,
}

/// One outline as it was last flattened.
#[derive(Debug)]
struct Flattened {
    /// The path, held so its address cannot be reused by another path.
    _path: Arc<kurbo::BezPath>,
    /// The coefficients of the placement the segments carry.
    placement: [f64; 6],
    /// The stroke style the outline was expanded with, for a stroke's outline.
    stroke: Option<kurbo::Stroke>,
    /// The outline.
    segments: Vec<Segment>,
    /// The frame that last asked for the outline.
    used: u64,
}

/// The outlines of recently drawn paths, by path, placement and stroke style.
///
/// A hit gives the same segments flattening would, since the segments are kept after the
/// placement was applied. One path drawn at several placements in a frame keeps an entry for
/// each.
// A menu opening over a drawing damages the drawing on every frame of the animation, and every one
// of those frames would flatten the same outlines again.
#[derive(Debug, Default)]
pub(crate) struct Outlines {
    held: FxHashMap<OutlineKey, SmallVec<[Flattened; 1]>>,
    frame: u64,
}

impl Outlines {
    /// How many frames an outline is kept after the last one that asked for it.
    const KEPT_FOR: u64 = 120;

    /// How many frames pass between sweeps of unused outlines.
    // A sweep walks the whole map.
    const SWEEP_EVERY: u64 = 64;

    /// Starts a frame, and drops the outlines no recent frame asked for.
    pub(crate) fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        if !self.frame.is_multiple_of(Self::SWEEP_EVERY) {
            return;
        }
        let now = self.frame;
        self.held.retain(|_, entries| {
            entries.retain(|entry| now.saturating_sub(entry.used) < Self::KEPT_FOR);
            !entries.is_empty()
        });
    }

    /// Replaces `into` with the outline of `path` under `placement`, filled for `stroke` when it
    /// is given, and reports whether a kept outline was reused.
    ///
    /// `tolerance` is in path units.
    pub(crate) fn outline(
        &mut self,
        into: &mut Vec<Segment>,
        path: &Arc<kurbo::BezPath>,
        stroke: Option<&kurbo::Stroke>,
        placement: Affine,
        tolerance: f64,
    ) -> bool {
        let key = OutlineKey {
            path: Arc::as_ptr(path) as usize,
            stroked: stroke.is_some(),
        };
        let coefficients = placement.as_coeffs();
        let now = self.frame;
        into.clear();
        let entries = self.held.entry(key).or_default();
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.placement == coefficients && entry.stroke.as_ref() == stroke)
        {
            entry.used = now;
            into.extend_from_slice(&entry.segments);
            return true;
        }

        match stroke {
            Some(style) => geometry::flatten_stroke(path, style, placement, tolerance, into),
            None => geometry::flatten(path, placement, tolerance, into),
        }
        let fresh = Flattened {
            _path: Arc::clone(path),
            placement: coefficients,
            stroke: stroke.cloned(),
            segments: into.clone(),
            used: now,
        };
        // An entry no draw of this frame used belongs to a placement the path has left.
        match entries.iter_mut().find(|entry| entry.used != now) {
            Some(stale) => *stale = fresh,
            None => entries.push(fresh),
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kurbo::{Affine, BezPath, Stroke};

    use super::Outlines;
    use crate::vector_coverage::geometry::TOLERANCE;

    fn curve() -> Arc<BezPath> {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.curve_to((10.0, 0.0), (20.0, 10.0), (20.0, 20.0));
        path.curve_to((20.0, 30.0), (10.0, 40.0), (0.0, 40.0));
        path.close_path();
        Arc::new(path)
    }

    #[test]
    fn the_same_geometry_in_the_same_place_is_flattened_once() {
        let mut outlines = Outlines::default();
        let mut into = Vec::new();
        let path = curve();

        outlines.begin_frame();
        assert!(!outlines.outline(&mut into, &path, None, Affine::IDENTITY, TOLERANCE));
        let first = into.clone();
        outlines.begin_frame();
        assert!(outlines.outline(&mut into, &path, None, Affine::IDENTITY, TOLERANCE));

        assert_eq!(into, first);
    }

    #[test]
    fn a_stroke_is_kept_apart_from_the_fill_and_follows_its_style() {
        let mut outlines = Outlines::default();
        let mut into = Vec::new();
        let path = curve();
        let thin = Stroke::new(1.0);
        let thick = Stroke::new(4.0);
        outlines.begin_frame();

        outlines.outline(&mut into, &path, None, Affine::IDENTITY, TOLERANCE);
        assert!(!outlines.outline(&mut into, &path, Some(&thin), Affine::IDENTITY, TOLERANCE));
        assert!(outlines.outline(&mut into, &path, Some(&thin), Affine::IDENTITY, TOLERANCE));
        assert!(!outlines.outline(&mut into, &path, Some(&thick), Affine::IDENTITY, TOLERANCE));
        assert!(outlines.outline(&mut into, &path, None, Affine::IDENTITY, TOLERANCE));
    }

    #[test]
    fn one_path_drawn_at_two_placements_keeps_both_outlines() {
        let mut outlines = Outlines::default();
        let mut into = Vec::new();
        let path = curve();
        let moved = Affine::translate((30.0, 0.0));

        for _ in 0..2 {
            outlines.begin_frame();
            outlines.outline(&mut into, &path, None, Affine::IDENTITY, TOLERANCE);
            outlines.outline(&mut into, &path, None, moved, TOLERANCE);
        }

        outlines.begin_frame();
        assert!(outlines.outline(&mut into, &path, None, Affine::IDENTITY, TOLERANCE));
        assert!(outlines.outline(&mut into, &path, None, moved, TOLERANCE));
    }

    #[test]
    fn a_new_path_with_the_same_shape_is_flattened_again() {
        let mut outlines = Outlines::default();
        let mut into = Vec::new();
        outlines.begin_frame();

        outlines.outline(&mut into, &curve(), None, Affine::IDENTITY, TOLERANCE);

        assert!(!outlines.outline(&mut into, &curve(), None, Affine::IDENTITY, TOLERANCE));
    }
}
