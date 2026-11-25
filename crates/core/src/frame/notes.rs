//! The resident-chunk notes a frame starts with.

use iceblink::{ChunkNote, Frame};

use crate::output::renderer::RendererState;
use crate::resident::Revisions;

/// Adds the notes that drop what the renderer no longer needs: everything after a reset, and
/// otherwise every resident revision retired since the renderer's last frame.
pub(crate) fn begin(
    frame: &mut Frame,
    renderer: &mut RendererState,
    revisions: &Revisions,
    resident: bool,
) {
    if renderer.fresh || renderer.reset {
        if resident {
            frame.chunk_notes.push(ChunkNote::ResetAll);
        }
        renderer.resident.clear();
        renderer.reset = false;
    } else {
        revisions.since(renderer.retired, |revision| {
            if renderer.resident.remove(&revision) {
                frame.chunk_notes.push(ChunkNote::Retired { revision });
            }
        });
    }
    renderer.retired = revisions.position();
}
