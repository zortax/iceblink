//! The links of the effect and order nodes a commit changed.

use iceblink::{EffectNode, Filter};

use crate::commit::journal::{Journal, Op, OrderChange};
use crate::commit::net::nets;
use crate::derive::Derive;
use crate::derive::links::{remove_from, renames};
use crate::scene::Scene;

/// Updates the clip users and shaded set of the effect nodes the commit changed, and marks the
/// inks of the order nodes with a written effect stale.
pub(super) fn effects(derive: &mut Derive<'_>, journal: &Journal) {
    let changes: Vec<(u32, Option<EffectNode>)> = nets(&journal.effects, &derive.scene.effects)
        .iter()
        .map(|net| (net.slot, net.before.cloned()))
        .collect();

    for (slot, before) in changes {
        let scene = &mut *derive.scene;
        let after = scene.effects.at(slot).map(|record| record.node.clone());
        renames(scene, before.as_ref(), after.as_ref());

        if let Some(clip) = before.as_ref().and_then(|node| node.clip) {
            remove_from(
                &mut scene.clips.at_mut(clip.slot()).map(|r| &mut r.effects),
                slot,
            );
        }
        if let Some(clip) = after.as_ref().and_then(|node| node.clip)
            && let Some(record) = scene.clips.at_mut(clip.slot())
        {
            record.effects.push(slot);
        }

        let shaded = after.as_ref().is_some_and(|node| {
            node.filters
                .iter()
                .any(|filter| matches!(filter, Filter::Shader { .. }))
        });
        if shaded {
            scene.shaded_effects.insert(slot);
        } else {
            scene.shaded_effects.remove(&slot);
        }

        if after.is_some() {
            // Every order node with the effect draws it differently now.
            let orders = scene
                .effects
                .at(slot)
                .map(|r| r.orders.clone())
                .unwrap_or_default();
            for order in orders {
                if let Some(record) = scene.orders.at_mut(order) {
                    record.dirty = true;
                }
            }
        }
    }
}

/// Marks the inks of the order nodes the commit changed or spliced stale, stops removed nodes
/// watching their scroll nodes, and updates the order users of their effects.
pub(super) fn orders(derive: &mut Derive<'_>, journal: &Journal) {
    for change in &journal.order_nodes {
        // A node created again after its removal has a later change of its own.
        if let Op::Removed(index) = change.op {
            if let Some(record) = journal.removed_order(index) {
                unwatch(derive.scene, change.slot, &record.watching);
            }
        } else if let Some(record) = derive.scene.orders.at_mut(change.slot) {
            record.dirty = true;
        }
    }

    // Order nodes without effects name nothing, so only their inks go stale.
    let changes = journal.effect_order_nets();

    for (slot, before) in changes {
        let scene = &mut *derive.scene;
        let after = scene.orders.at(slot).map(|record| record.node);
        renames(scene, before.as_ref(), after.as_ref());

        if let Some(effect) = before.and_then(|node| node.effect) {
            remove_from(
                &mut scene.effects.at_mut(effect.slot()).map(|r| &mut r.orders),
                slot,
            );
        }
        if let Some(effect) = after.and_then(|node| node.effect)
            && let Some(record) = scene.effects.at_mut(effect.slot())
        {
            record.orders.push(slot);
        }
    }

    spliced_dirty(derive, journal);
}

/// Marks the inks of every order node the commit spliced stale.
fn spliced_dirty(derive: &mut Derive<'_>, journal: &Journal) {
    for change in &journal.orders {
        if let OrderChange::Splice(splice) = change
            && let Some(record) = derive.scene.orders.at_mut(splice.node)
        {
            record.dirty = true;
        }
    }
}

/// Removes order node `slot` from the watchers of every scroll node in `scrolls`.
pub(crate) fn unwatch(scene: &mut Scene, slot: u32, scrolls: &[u32]) {
    for &scroll in scrolls {
        if let Some(record) = scene.scrolls.at_mut(scroll) {
            record.watchers.remove(&slot);
        }
    }
}
