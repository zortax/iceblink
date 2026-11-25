//! The frame slots of the paints, parameter blocks and shaders a commit changed.

use std::sync::Arc;

use smallvec::SmallVec;

use crate::commit::journal::Journal;
use crate::commit::net::nets;
use crate::derive::Derive;
use crate::scene::record::NO_FRAME_SLOT;
use crate::tables::paint::frame_paint;

/// Allocates, writes and releases the frame slots of the paints the commit changed.
pub(super) fn paints(derive: &mut Derive<'_>, journal: &Journal) {
    let changes: SmallVec<[(u32, bool, u32); 4]> = nets(&journal.paints, &derive.scene.paints)
        .iter()
        .map(|net| {
            (
                net.slot,
                net.before.is_some() && !net.created,
                net.links.map_or(NO_FRAME_SLOT, |l| l.frame),
            )
        })
        .collect();

    for (slot, written, frame) in changes {
        let record = derive.scene.paints.at_mut(slot);
        if frame != NO_FRAME_SLOT && (record.is_none() || !written) {
            derive.tables.paints.release(frame, derive.serial);
        }
        let Some(record) = record else {
            continue;
        };

        let paint = frame_paint(&record.paint);
        if written {
            derive.tables.paints.write(record.links.frame, paint);
        } else {
            record.links.frame = derive.tables.paints.allocate(paint);
        }
    }
}

/// Allocates, writes and releases the frame slots of the parameter blocks the commit changed.
pub(super) fn params(derive: &mut Derive<'_>, journal: &Journal) {
    let changes: SmallVec<[(u32, bool, u32); 4]> = nets(&journal.params, &derive.scene.params)
        .iter()
        .map(|net| {
            (
                net.slot,
                net.before.is_some() && !net.created,
                net.links.map_or(NO_FRAME_SLOT, |l| l.frame),
            )
        })
        .collect();

    for (slot, written, frame) in changes {
        let record = derive.scene.params.at_mut(slot);
        if frame != NO_FRAME_SLOT && (record.is_none() || !written) {
            derive.tables.params.release(frame, derive.serial);
        }
        let Some(record) = record else {
            continue;
        };

        if written {
            derive.tables.params.write(record.links.frame, record.block);
        } else {
            record.links.frame = derive.tables.params.allocate(record.block);
        }
    }
}

/// Gives every shader the commit created a frame number, and removes the shaders it removed from
/// the frame tables.
pub(super) fn shaders(derive: &mut Derive<'_>, journal: &Journal) {
    let changes: SmallVec<[(u32, u32); 4]> = nets(&journal.shaders, &derive.scene.shaders)
        .iter()
        .map(|net| (net.slot, net.links.map_or(NO_FRAME_SLOT, |l| l.frame)))
        .collect();

    // Shaders are never written, so a shader that held a frame number before the commit was
    // removed by it.
    for (slot, frame) in changes {
        if frame != NO_FRAME_SLOT {
            derive.tables.shaders.remove(&frame);
        }
        if let Some(record) = derive.scene.shaders.at_mut(slot) {
            let frame = derive.tables.next_shader;
            derive.tables.next_shader += 1;
            record.links.frame = frame;
            derive
                .tables
                .shaders
                .insert(frame, Arc::clone(&record.desc));
        }
    }
}
