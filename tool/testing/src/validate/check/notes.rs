//! Chunk notes and the resident chunks they change.

use iceblink::{ChunkNote, Frame};

use crate::validate::Rule;
use crate::validate::check::Report;
use crate::validate::known::{Known, Lookup};

/// Applies the frame's chunk notes in order, checking each against the resident chunks.
pub(crate) fn apply(frame: &Frame, known: &mut Known, report: &mut Report) {
    for (index, note) in frame.chunk_notes.iter().enumerate() {
        match note {
            ChunkNote::Inserted { revision, chunk } => {
                if known.retired.contains(revision) {
                    report.push(
                        Rule::ChunkNotes,
                        format!("note {index} inserts revision {revision}, which was retired"),
                    );
                }
                if let Lookup::Found(()) = known.resident.insert(*revision, chunk.clone()) {
                    report.push(
                        Rule::ChunkNotes,
                        format!("note {index} inserts revision {revision}, which is resident"),
                    );
                }
            }
            ChunkNote::Retired { revision } => {
                if known.resident.remove(*revision).is_missing() {
                    report.push(
                        Rule::ChunkNotes,
                        format!("note {index} retires revision {revision}, which is not resident"),
                    );
                }
                known.retired.insert(*revision);
            }
            ChunkNote::ResetAll => known.resident.clear(),
        }
    }
}
