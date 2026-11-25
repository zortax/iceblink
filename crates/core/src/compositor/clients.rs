//! Registering and removing clients, and handing out their transactions.

use iceblink::{ClientId, IdSpace, Transaction};

use crate::client::Client;
use crate::compositor::CompositorImpl;
use crate::frame::lower::retire;
use crate::scene::Scene;
use crate::shared_content::image_key;

impl CompositorImpl {
    pub(crate) fn add_client(&mut self) -> (ClientId, IdSpace) {
        let id = ClientId(self.next_client);
        self.next_client += 1;

        let client = Client::new(id);
        let ids = client.ids.clone();
        self.clients.insert(id, client);
        (id, ids)
    }

    pub(crate) fn drop_client(&mut self, id: ClientId) {
        let Some(mut client) = self.clients.remove(&id) else {
            return;
        };
        self.release_scene(&mut client.scene);
        self.content.cache.forget_client(id);

        for output in self.outputs.values_mut() {
            if output.root.is_some_and(|(owner, _)| owner == id) {
                output.root = None;
                output.damage_all();
            }
        }
    }

    pub(crate) fn client_transaction(&mut self, id: ClientId) -> Transaction {
        match self.clients.get_mut(&id) {
            Some(client) => client.transaction(),
            // A transaction for an unknown client is refused when it is committed.
            None => Transaction::new(IdSpace::new(id)),
        }
    }

    /// Releases every frame slot and lowering a removed client's scene held.
    fn release_scene(&mut self, scene: &mut Scene) {
        let serial = self.serial;
        let tables = &mut self.tables;
        scene
            .transforms
            .each(|_, _, record| tables.spaces.release(record.links.frame, serial));
        scene
            .clips
            .each(|_, _, record| tables.clips.release(record.links.frame, serial));
        scene
            .paints
            .each(|_, _, record| tables.paints.release(record.links.frame, serial));
        scene
            .params
            .each(|_, _, record| tables.params.release(record.links.frame, serial));
        scene.shaders.each(|_, _, record| {
            tables.shaders.remove(&record.links.frame);
        });

        let content = &mut self.content;
        let client = scene.client;
        scene
            .images
            .each(|slot, _, _| content.release_image(image_key(client, slot)));

        for record in scene.chunks.values_mut() {
            for &slot in &record.derived.local_frames {
                tables.clips.release(slot, serial);
            }
            tables.release_inline(&record.derived.inline, serial);
            if let Some(lowered) = record.derived.lowered.take() {
                retire(lowered, &mut self.revisions, content, tables);
            }
        }
    }
}
