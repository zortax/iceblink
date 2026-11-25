//! Image writes and texture frames, applied once the commit succeeds.

mod masks;

use crate::commit::journal::{Journal, Op};
use crate::derive::Derive;
use crate::derive::chunks::damage_users;
use crate::derive::route::Routes;
use crate::scene::kind::{Kind, ObjectRef};
use crate::shared_content::image_key;

use self::masks::{damage_effects, damage_local, masked_clips};

/// Creates, removes and writes the pixels of the images the commit created, removed and wrote.
pub(crate) fn create(derive: &mut Derive<'_>, journal: &Journal) {
    let client = derive.scene.client;
    for change in &journal.images {
        let key = image_key(client, change.slot);
        match &change.op {
            Op::Created(_) => {
                if let Some(record) = derive.scene.images.at(change.slot) {
                    derive.content.create_image(key, record.desc);
                }
            }
            Op::Removed(_) => derive.content.release_image(key),
            Op::Wrote(_) => {}
        }
    }
    for (id, write) in &journal.deferred.image_writes {
        if derive
            .scene
            .images
            .get(id.slot(), id.generation())
            .is_some()
        {
            derive
                .content
                .write_image(image_key(client, id.slot()), write);
        }
    }
}

/// Damages what draws the written images and the external textures that have new content.
pub(crate) fn apply(derive: &mut Derive<'_>, journal: &Journal, routes: &Routes) {
    for (id, _) in &journal.deferred.image_writes {
        let Some(record) = derive.scene.images.get(id.slot(), id.generation()) else {
            continue;
        };
        let direct = record.masks.clone();
        let masks = masked_clips(derive.scene, &direct);
        damage_users(derive, ObjectRef::new(Kind::Image, id.slot()), routes);
        damage_local(derive, id.slot(), routes);
        for &clip in &masks {
            damage_users(derive, ObjectRef::new(Kind::Clip, clip), routes);
        }
        damage_effects(derive, &masks, routes);
    }

    for id in &journal.deferred.texture_frames {
        damage_users(derive, ObjectRef::new(Kind::Texture, id.slot()), routes);
    }
}
