//! Appending instances to lanes, and the batches that draw them.

use iceblink::{Batch, BatchKind, DeviceRect, ItemSource, Lane, Lanes};

use crate::naive::frame::builder::Builder;
use crate::naive::geometry::{Bounds, output_rect};

impl Builder<'_> {
    /// Returns the whole output.
    pub(crate) fn output_rect(&self) -> DeviceRect {
        output_rect(self.env.config.size)
    }

    /// Returns the output grown by what the open groups' filters read past it.
    pub(crate) fn reached(&self) -> Bounds {
        let reach = self.reach;
        let output = self.env.output;
        Bounds {
            x0: output.x0 - reach,
            y0: output.y0 - reach,
            x1: output.x1 + reach,
            y1: output.y1 + reach,
        }
    }

    /// Returns the ink of every batch: the output, grown inside groups by what their filters
    /// read past it.
    fn batch_ink(&self) -> DeviceRect {
        self.reached()
            .round_out()
            .unwrap_or_else(|| self.output_rect())
    }

    /// Appends a batch that draws one object, such as a group marker or a vector pass.
    pub(crate) fn batch(&mut self, kind: BatchKind) {
        let ink = self.batch_ink();
        self.frame.batches.push(Batch {
            kind,
            range: 0..0,
            ink,
        });
    }

    /// Starts a new item, so its instances get batches of their own.
    pub(crate) fn next_item(&mut self) {
        self.item += 1;
    }

    /// Appends `instance` to a lane as a transient run of one, and draws it with a batch of
    /// `kind`.
    ///
    /// The batch extends the last one when that drew the same item's previous instance of the
    /// same lane and binding.
    pub(crate) fn instance<T>(
        &mut self,
        lane: impl FnOnce(&mut Lanes) -> &mut Lane<T>,
        instance: T,
        kind: BatchKind,
    ) {
        let lane = lane(&mut self.frame.lanes);
        let position = lane.sources.len() as u32;
        lane.sources
            .push(ItemSource::transient(lane.items.len() as u32));
        lane.items.push(instance);
        self.counters.items_drawn += 1;

        if self.batched_item == self.item
            && let Some(last) = self.frame.batches.last_mut()
            && last.kind == kind
            && last.range.end == position
        {
            last.range.end += 1;
            return;
        }

        let ink = self.batch_ink();
        self.frame.batches.push(Batch {
            kind,
            range: position..position + 1,
            ink,
        });
        self.batched_item = self.item;
    }
}
