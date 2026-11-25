//! Memory, budgets and dropping the caches the compositor rebuilds on demand.

use iceblink::{CompositorBudgets, CompositorMemory};

use crate::compositor::CompositorImpl;
use crate::frame::lower::retire;

impl CompositorImpl {
    pub(crate) fn memory_report(&self) -> CompositorMemory {
        let mut memory = CompositorMemory {
            table_bytes: self.tables.bytes(),
            ..CompositorMemory::default()
        };

        for client in self.clients.values() {
            memory.chunk_bytes += client.scene.chunk_bytes;
        }

        memory.image_bytes = self.content.images.bytes();
        let (atlas, pinned) = self.content.atlas_bytes();
        memory.atlas_bytes = atlas;
        memory.atlas_pinned_bytes = pinned;
        memory
    }

    pub(crate) fn trim_atlas(&mut self, to_bytes: u64) -> u64 {
        let freed = self.content.cache.trim(to_bytes);
        self.content.drain();
        freed
    }

    pub(crate) fn forget(&mut self) -> u64 {
        for client in self.clients.values_mut() {
            for record in client.scene.chunks.values_mut() {
                if let Some(lowered) = record.derived.lowered.take() {
                    retire(
                        lowered,
                        &mut self.revisions,
                        &mut self.content,
                        &mut self.tables,
                    );
                }
            }
        }

        let freed = self.content.forget();
        crate::shared_content::masks::refresh(
            &mut self.clients,
            &mut self.tables,
            &mut self.content,
        );
        for client in self.clients.values_mut() {
            client.scene.recount_all();
        }

        for output in self.outputs.values_mut() {
            output.renderer.reset = true;
            output.damage_all();
        }
        freed
    }

    pub(crate) fn set_budget_limits(&mut self, budgets: CompositorBudgets) {
        self.budgets = budgets;
    }
}
