//! Creating and writing the paint and parameter slots.

use iceblink::{CommitError, Paint, PaintId, ParamsBlock, ParamsId, RejectReason};

use crate::commit::apply::Apply;
use crate::commit::apply::journaled::{create, write};
use crate::commit::finite;
use crate::scene::record::{Links, PaintRecord, ParamsRecord};

impl Apply<'_> {
    pub(crate) fn create_paint(&mut self, id: PaintId, paint: Paint) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.paints, id.slot(), id.generation())?;
        self.require(finite::paint(&paint), RejectReason::InvalidItem)?;

        let record = PaintRecord {
            paint,
            links: Links::default(),
        };
        create(
            self.command,
            &mut self.scene.paints,
            &mut self.journal.paints,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn set_paint(&mut self, id: PaintId, paint: Paint) -> Result<(), CommitError> {
        let Some(record) = self.scene.paints.get(id.slot(), id.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        self.require(
            record.paint.kind() == paint.kind(),
            RejectReason::PaintKindChanged,
        )?;
        self.require(finite::paint(&paint), RejectReason::InvalidItem)?;

        write(
            self.command,
            &mut self.scene.paints,
            &mut self.journal.paints,
            id.slot(),
            id.generation(),
            paint,
        );
        self.counters.paint_writes += 1;
        Ok(())
    }

    pub(crate) fn create_params(
        &mut self,
        id: ParamsId,
        block: ParamsBlock,
    ) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.params, id.slot(), id.generation())?;
        self.require(finite::params(&block), RejectReason::InvalidItem)?;

        let record = ParamsRecord {
            block,
            links: Links::default(),
        };
        create(
            self.command,
            &mut self.scene.params,
            &mut self.journal.params,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn set_params(
        &mut self,
        id: ParamsId,
        block: ParamsBlock,
    ) -> Result<(), CommitError> {
        self.require(self.live_params(id), RejectReason::StaleId)?;
        self.require(finite::params(&block), RejectReason::InvalidItem)?;

        let written = write(
            self.command,
            &mut self.scene.params,
            &mut self.journal.params,
            id.slot(),
            id.generation(),
            block,
        );
        self.require(written, RejectReason::StaleId)?;
        self.counters.paint_writes += 1;
        Ok(())
    }
}
