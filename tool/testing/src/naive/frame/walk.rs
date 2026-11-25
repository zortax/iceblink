//! The order tree walk: paint order, groups and the clips of effects.

use iceblink::{BatchKind, EffectNode, FrameFilter, FrameGroup, OrderEntry, OrderId};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::chain::{self, Link};
use crate::naive::frame::checkpoint::Checkpoint;
use crate::naive::frame::item;
use crate::naive::frame::lower::external::read_extent;
use crate::naive::geometry::Bounds;

/// Lowers the subtree of the order node `id` in paint order, and returns its device ink.
pub(crate) fn node(builder: &mut Builder<'_>, id: OrderId) -> Bounds {
    let client = builder.env.client;
    let Some(order) = client.orders.get(id) else {
        return Bounds::EMPTY;
    };

    builder.counters.order_nodes_visited += 1;
    let effect = order
        .node
        .effect
        .and_then(|effect| client.effects.get(effect));
    match effect {
        Some(effect) if effect.isolates() => group(builder, &order.entries, effect),
        Some(effect) => {
            let held = builder.effect_clips.len();
            if let Some(clip) = effect.clip {
                chain::node_chain(client, clip, &mut builder.effect_clips);
            }
            let ink = entries(builder, &order.entries);
            builder.effect_clips.truncate(held);
            ink
        }
        None => entries(builder, &order.entries),
    }
}

fn entries(builder: &mut Builder<'_>, entries: &[OrderEntry]) -> Bounds {
    let mut ink = Bounds::EMPTY;
    for entry in entries {
        let entry_ink = match entry {
            OrderEntry::Chunk(key) => item::chunk(builder, *key),
            OrderEntry::Node(child) => node(builder, *child),
        };
        ink = ink.union(entry_ink);
    }

    ink
}

/// Lowers the subtree of an isolating effect into a group, and returns the group's written
/// region.
///
/// The clips of the effects above that do not isolate are applied when the group is
/// composited, together with the effect's own clip, and not to the content inside it. A group
/// with an empty written region is left out with its content.
fn group(builder: &mut Builder<'_>, entries_of: &[OrderEntry], effect: &EffectNode) -> Bounds {
    let checkpoint = Checkpoint::take(builder);

    let mut composite: Vec<Link> = std::mem::take(&mut builder.effect_clips);
    let outer = composite.len();
    if let Some(clip) = effect.clip {
        chain::node_chain(builder.env.client, clip, &mut composite);
    }

    let filters = builder.filters(&effect.filters);
    let clip = builder.clip(&composite);
    // The written region is cut to the effect's own clip only, so the filters read the content
    // that the clips above cut away.
    let clip_box = chain::chain_box(&composite[outer..], &mut builder.worlds);
    composite.truncate(outer);

    let index = builder.frame.groups.len() as u32;
    builder.frame.groups.push(FrameGroup {
        bounds: [0.0; 4],
        source: [0.0; 4],
        clip,
        opacity: effect.opacity.clamp(0.0, 1.0),
        blend: effect.blend,
        filters: filters.clone(),
    });
    builder.batch(BatchKind::GroupBegin(index));

    let reach = builder.reach;
    builder.groups_open += 1;
    builder.reach += widest_support(&filters);
    let content = entries(builder, entries_of);
    builder.groups_open -= 1;
    builder.reach = reach;
    builder.effect_clips = composite;

    // A nested group is cut by the source of the group around it, which holds its written
    // region.
    let source = if content.is_empty() {
        Bounds::EMPTY
    } else {
        Bounds::of(read_extent(content.to_rect(), &filters))
    };
    let written = match builder.groups_open {
        0 => source.intersect(clip_box).intersect(builder.env.output),
        _ => source.intersect(clip_box),
    };

    if written.is_empty() {
        checkpoint.restore(builder);
        return Bounds::EMPTY;
    }

    builder.frame.groups[index as usize].bounds = written.to_rect();
    builder.frame.groups[index as usize].source = source.to_rect();
    builder.batch(BatchKind::GroupEnd(index));
    builder.counters.groups += 1;
    written
}

/// Returns how far a filter chain reads past the pixels it writes, on its widest side.
fn widest_support(filters: &[FrameFilter]) -> f32 {
    filters
        .iter()
        .map(|filter| {
            let [left, top, right, bottom] = filter.support();
            left.max(top).max(right).max(bottom)
        })
        .sum()
}
