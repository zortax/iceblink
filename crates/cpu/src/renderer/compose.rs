//! Applying a frame's work and shifts, and redrawing its damage.

use iceblink::Frame;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};

use crate::draw::Scene;
use crate::frame::backdrop::nested_reach;
use crate::frame::rects::draw_rects;
use crate::frame::{Context, Counts};
use crate::renderer::{CpuRenderer, MAX_TEXTURE_SIZE, shift};
use crate::target::{Surface, TexelsMut};
use crate::vector::Inputs;

/// What composing a frame did.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Composed {
    pub(super) counts: Counts,
    /// Pixels redrawn.
    pub(super) redrawn: u64,
}

impl CpuRenderer {
    /// Applies the frame's page work, chunk notes and table deltas, and returns the bytes the
    /// page work wrote.
    pub(super) fn apply_work(&mut self, frame: &Frame) -> u64 {
        self.frames += 1;
        let written = self.pages.apply(&frame.resources.pages, MAX_TEXTURE_SIZE);
        self.resident.apply(&frame.chunk_notes);
        self.tables.apply(&frame.tables);
        written
    }

    /// Applies the frame's shifts to the kept target unless `image` is set, then redraws `draw`
    /// into the kept target or into presenter image `image`.
    ///
    /// A `full` redraw starts at no opaque cover.
    pub(super) fn compose(
        &mut self,
        frame: &Frame,
        draw: &DamageSet,
        full: bool,
        image: Option<u32>,
    ) -> Composed {
        if image.is_none() {
            let mut scratch = Vec::new();
            for held in &frame.shifts {
                shift::apply(&mut self.kept, *held, &mut scratch);
            }
        }

        self.instances.resolve(frame, &self.resident);
        let scene = Scene {
            spaces: self.tables.spaces(),
            clips: self.tables.clips(),
            paints: self.tables.paints(),
            pages: &self.pages,
            gamma: self.gamma,
            blue_to_red: self.subpixel_order == iceblink::SubpixelOrder::BlueToRed,
        };
        self.bins
            .fill(&scene, &frame.mark_plan, &self.instances.marks);
        let inputs = Inputs {
            paths: &frame.paths,
            clips: self.tables.entries(),
            paints: self.tables.paints(),
            spaces: self.tables.matrices(),
        };
        let rasters = if frame.vector_plan.is_empty() {
            Vec::new()
        } else {
            self.vectors.rasterise(&frame.vector_plan, &inputs)
        };
        let nested = nested_reach(frame);
        // A cover is its fill inset by a pixel, so none holds a frame that redraws everything.
        let covers = if full || draw.is_full() {
            &[][..]
        } else {
            &frame.opaque_covers[..]
        };

        let pixmap = match image {
            Some(image) => match self.presenter.image_mut(image) {
                Some(pixmap) => pixmap,
                None => return Composed::default(),
            },
            None => &mut self.kept,
        };
        let used = Rect::new(
            Point::new(0, 0),
            Size::new(pixmap.width() as i32, pixmap.height() as i32),
        );
        let rects = draw_rects(frame, draw, used);
        let redrawn = rects
            .iter()
            .map(|rect| rect.width() as u64 * rect.height() as u64)
            .sum();
        let context = Context {
            frame,
            scene,
            instances: &self.instances,
            bins: &self.bins,
            rasters: &rasters,
            externals: &self.externals,
            used,
            nested: &nested,
            covers,
        };
        let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
        let mut surface = Surface::new(
            Point::new(0, 0),
            width,
            height,
            TexelsMut::Byte(pixmap.texels_mut()),
        );

        for worker in &mut self.workers {
            worker.counts = Counts::default();
        }
        crate::frame::draw(&context, &mut surface, &rects, &mut self.workers);
        let mut counts = Counts::default();
        for worker in &self.workers {
            counts.draws += worker.counts.draws;
            counts.culled += worker.counts.culled;
        }
        Composed { counts, redrawn }
    }
}
