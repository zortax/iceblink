//! Recording, moving and removing chunks.

use iceblink::{ChunkKey, ChunkProps, CommitError, RecordedChunk, RejectReason};

use crate::commit::apply::Apply;
use crate::commit::journal::ChunkChange;
use crate::frame::lower::recorded_bytes;
use crate::scene::chunk::ChunkRecord;

impl Apply<'_> {
    pub(crate) fn record(&mut self, chunk: RecordedChunk) -> Result<(), CommitError> {
        self.check_props(&chunk.props)?;
        for clip in &chunk.local_clips {
            self.check_local_clip(clip)?;
        }
        for chain in &chunk.filters {
            self.check_filter_ids(chain)?;
        }
        self.require(chunk.props.flags.is_valid(), RejectReason::InvalidItem)?;

        self.counters.bytes_recorded += recorded_bytes(&chunk);
        self.counters.chunks_recorded += 1;

        let key = chunk.key;
        let derived = self.scene.spare_derived.pop().unwrap_or_default();
        let before = self
            .scene
            .chunks
            .insert(key, ChunkRecord::new(chunk, derived));
        if let Some(record) = &before {
            self.scene.uncount(record);
        }

        let before = self.journal.keep(before);
        self.journal.chunks.push(ChunkChange::Replaced {
            key,
            command: self.command,
            before,
        });
        Ok(())
    }

    pub(crate) fn set_props(
        &mut self,
        key: ChunkKey,
        props: ChunkProps,
    ) -> Result<(), CommitError> {
        self.check_props(&props)?;
        self.require(
            self.scene.chunks.contains_key(&key),
            RejectReason::UnknownChunk,
        )?;
        self.require(props.flags.is_valid(), RejectReason::InvalidItem)?;

        let Some(record) = self.scene.chunks.get_mut(&key) else {
            return Err(self.reject(RejectReason::UnknownChunk));
        };
        let before = core::mem::replace(&mut record.recorded.props, props);
        self.journal.chunks.push(ChunkChange::Props { key, before });
        Ok(())
    }

    pub(crate) fn set_offset(
        &mut self,
        key: ChunkKey,
        offset: [f32; 2],
    ) -> Result<(), CommitError> {
        let finite = offset.iter().all(|value| value.is_finite());
        let Some(record) = self.scene.chunks.get_mut(&key) else {
            return Err(self.reject(RejectReason::UnknownChunk));
        };
        if !finite {
            return Err(self.reject(RejectReason::InvalidItem));
        }

        let before = core::mem::replace(&mut record.offset, offset);
        self.journal
            .chunks
            .push(ChunkChange::Offset { key, before });
        self.counters.offsets_written += 1;
        Ok(())
    }

    pub(crate) fn remove_chunk(&mut self, key: ChunkKey) -> Result<(), CommitError> {
        let Some(before) = self.scene.chunks.remove(&key) else {
            return Err(self.reject(RejectReason::UnknownChunk));
        };
        self.scene.uncount(&before);

        let before = self.journal.keep(Some(before));
        self.journal.chunks.push(ChunkChange::Replaced {
            key,
            command: self.command,
            before,
        });
        self.counters.chunks_removed += 1;
        Ok(())
    }
}
