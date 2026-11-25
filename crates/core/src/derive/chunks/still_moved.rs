//! Order nodes whose inks follow the still pixels of carried content.

use crate::derive::chunks::Touched;
use crate::scene::Scene;

/// Adds to `touched` the order nodes whose inks depend on the still pixels of the content that
/// the scroll nodes `scrolls` carry, once those pixels moved under carries that refer to a clip
/// node.
///
/// These are the scroll nodes' watchers, which hold carried entries without being carried, and
/// the carried nodes with an effect, whose effect decides whether they move with their entries.
pub(super) fn still_moved(scene: &Scene, scrolls: &[u32], touched: &mut Touched) {
    if scrolls.is_empty() {
        return;
    }
    for &scroll in scrolls {
        if let Some(record) = scene.scrolls.at(scroll) {
            touched.nodes.extend(record.watchers.iter().copied());
        }
    }
    scene.effects.each(|_, _, effect| {
        for &node in &effect.orders {
            let carried = scene.orders.at(node).is_some_and(|record| {
                record
                    .ink
                    .carry()
                    .is_some_and(|carry| scrolls.contains(&carry.scroll))
            });
            if carried {
                touched.nodes.push(node);
            }
        }
    });
}
