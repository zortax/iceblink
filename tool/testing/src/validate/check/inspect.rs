//! Lookups of frame slots, tiles and filters against what the renderer holds.

use std::collections::BTreeSet;

use iceblink::{
    AtlasTile, FRAME_NO_CLIP, Frame, FrameClipShape, FrameFilter, FramePaint, PageFormat,
    PaintKind, PaintRef,
};

use crate::validate::Rule;
use crate::validate::check::Report;
use crate::validate::known::{Known, Lookup};

/// A frame under inspection, after its work and deltas were applied to what the renderer holds.
///
/// Every lookup reports a slot the renderer is known to lack, and lets a slot it may hold pass.
pub(crate) struct Inspect<'a> {
    pub(crate) frame: &'a Frame,
    pub(crate) known: &'a Known,
    pub(crate) report: Report,
    /// Clip slots whose links were checked already.
    checked_clips: BTreeSet<u32>,
}

impl<'a> Inspect<'a> {
    pub(crate) fn new(frame: &'a Frame, known: &'a Known, report: Report) -> Self {
        Self {
            frame,
            known,
            report,
            checked_clips: BTreeSet::new(),
        }
    }

    pub(crate) fn push(&mut self, rule: Rule, detail: impl Into<String>) {
        self.report.push(rule, detail);
    }

    /// Checks a frame clip slot, which may be `FRAME_NO_CLIP`, and the links of its chain.
    pub(crate) fn clip(&mut self, slot: u32, at: impl Fn() -> String) {
        if slot == FRAME_NO_CLIP {
            return;
        }

        let entry = match self.known.clips.get(slot) {
            Lookup::Found(entry) => entry,
            Lookup::Missing => {
                self.push(
                    Rule::Slots,
                    format!("{} names clip slot {slot}, which is empty", at()),
                );
                return;
            }
            Lookup::Unknown => return,
        };
        if !self.checked_clips.insert(slot) {
            return;
        }

        for (index, link) in entry.links.iter().enumerate() {
            let link_at = || format!("link {index} of clip slot {slot}");
            self.space(link.space, link_at);
            if let FrameClipShape::Mask { tile, mode, .. } = &link.shape {
                if !mode.is_valid() {
                    self.push(
                        Rule::Values,
                        format!("{} has mask mode {}", link_at(), mode.0),
                    );
                }
                self.tile(*tile, None, link_at);
            }
        }
    }

    /// Checks a frame space slot.
    pub(crate) fn space(&mut self, slot: u32, at: impl Fn() -> String) {
        if self.known.spaces.get(slot).is_missing() {
            self.push(
                Rule::Slots,
                format!("{} names space slot {slot}, which is empty", at()),
            );
        }
    }

    /// Checks a paint reference: its kind, its slot, and that the slot holds a paint of its kind.
    pub(crate) fn paint(&mut self, paint: PaintRef, at: impl Fn() -> String) {
        if !paint.kind.is_valid() {
            self.push(
                Rule::Values,
                format!("{} has paint kind {}", at(), paint.kind.0),
            );
            return;
        }
        if paint.kind == PaintKind::INLINE {
            self.push(
                Rule::Values,
                format!("{} has an inline colour, which frames never hold", at()),
            );
            return;
        }
        if paint.kind == PaintKind::NONE {
            return;
        }

        let slot = paint.index;
        match self.known.paints.get(slot) {
            Lookup::Found(held) => {
                let kind = match held {
                    FramePaint::None => PaintKind::NONE,
                    FramePaint::Solid(_) => PaintKind::SOLID,
                    FramePaint::Gradient(_) => PaintKind::GRADIENT,
                    FramePaint::Sides(_) => PaintKind::SIDES,
                };
                if kind != paint.kind {
                    self.push(
                        Rule::PaintKinds,
                        format!(
                            "{} names paint slot {slot} as kind {}, which holds kind {}",
                            at(),
                            paint.kind.0,
                            kind.0
                        ),
                    );
                }
            }
            Lookup::Missing => {
                self.push(
                    Rule::Slots,
                    format!("{} names paint slot {slot}, which is empty", at()),
                );
            }
            Lookup::Unknown => {}
        }
    }

    /// Checks a frame parameter slot.
    pub(crate) fn params(&mut self, slot: u32, at: impl Fn() -> String) {
        if self.known.params.get(slot).is_missing() {
            self.push(
                Rule::Slots,
                format!("{} names parameter slot {slot}, which is empty", at()),
            );
        }
    }

    /// Checks a frame shader slot.
    pub(crate) fn shader(&mut self, slot: u32, at: impl Fn() -> String) {
        if self.known.shaders.get(slot).is_missing() {
            self.push(
                Rule::Slots,
                format!("{} names shader slot {slot}, which is not live", at()),
            );
        }
    }

    /// Checks that `tile` lies on a live page, of `format` when one is given.
    pub(crate) fn tile(
        &mut self,
        tile: AtlasTile,
        format: Option<PageFormat>,
        at: impl Fn() -> String,
    ) {
        let page = match self.known.pages.get(tile.page) {
            Lookup::Found(page) => *page,
            Lookup::Missing => {
                self.push(
                    Rule::Slots,
                    format!("{} names page {}, which does not exist", at(), tile.page),
                );
                return;
            }
            Lookup::Unknown => return,
        };

        if let Some(format) = format
            && page.format != format
        {
            self.push(
                Rule::Tiles,
                format!(
                    "{} reads page {} of format {} as format {}",
                    at(),
                    tile.page,
                    page.format.0,
                    format.0
                ),
            );
        }

        let [x, y, width, height] = tile.rect.map(i64::from);
        let inside = x + width <= i64::from(page.size[0]) && y + height <= i64::from(page.size[1]);
        if !inside {
            self.push(
                Rule::Tiles,
                format!(
                    "{} reads {:?} outside page {} of {}x{}",
                    at(),
                    tile.rect,
                    tile.page,
                    page.size[0],
                    page.size[1]
                ),
            );
        }
    }

    /// Checks that `page` is a live page of `format`.
    pub(crate) fn page(&mut self, page: u32, format: PageFormat, at: impl Fn() -> String) {
        self.tile(AtlasTile { page, rect: [0; 4] }, Some(format), at);
    }

    /// Checks the shader and parameter slots of a filter chain.
    pub(crate) fn filters(&mut self, filters: &[FrameFilter], at: impl Fn() -> String) {
        for (index, filter) in filters.iter().enumerate() {
            if let FrameFilter::Shader { shader, params, .. } = filter {
                let filter_at = || format!("filter {index} of {}", at());
                self.shader(*shader, filter_at);
                self.params(*params, filter_at);
            }
        }
    }
}
