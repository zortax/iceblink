//! Drawing text again after the rasteriser or the font registry changes.

use iceblink::ChunkKey;

use crate::commit::run_output;
use crate::compositor::CompositorImpl;
use crate::derive::chunks::set_inks;
use crate::derive::inks;
use crate::derive::order::refresh_chunk_parents;
use crate::derive::runs::{Measure, refresh};
use crate::shared_content::prefetch::prefetch;

impl CompositorImpl {
    /// Drops every atlas entry and lowering and measures every glyph run again with the current
    /// rasteriser and registry, if any client has chunks.
    pub(crate) fn restart_text(&mut self) {
        let drawn = self
            .clients
            .values()
            .any(|client| !client.scene.chunks.is_empty());
        if drawn {
            self.forget();
            self.measure_text();
        }
    }

    /// Measures the ink of every glyph run again, then the inks of the chunks that hold them and
    /// of the order nodes above those.
    fn measure_text(&mut self) {
        for (&id, client) in &mut self.clients {
            let scene = &mut client.scene;
            let keys: Vec<ChunkKey> = scene
                .chunks
                .iter()
                .filter(|(_, record)| !record.recorded.glyph_runs.is_empty())
                .map(|(key, _)| *key)
                .collect();
            if keys.is_empty() {
                continue;
            }

            let text = run_output(&self.outputs, id);
            self.content.scale = text.scale;
            prefetch(&mut self.content, scene, &keys, text);

            for &key in &keys {
                if let Some(record) = scene.chunks.get_mut(&key) {
                    record.derived.runs.clear();
                }

                let mut measure = Measure {
                    scene,
                    tables: &mut self.tables,
                    content: &mut self.content,
                    outputs: &self.outputs,
                    text,
                };
                refresh(&mut measure, key);

                let Some(record) = scene.chunks.get(&key) else {
                    continue;
                };
                let measured = inks::measure(scene, record, &mut self.work, Vec::new());
                set_inks(scene, key, measured);
                scene.recount(key);
            }

            refresh_chunk_parents(scene, &keys, &mut self.work);
        }
    }
}
