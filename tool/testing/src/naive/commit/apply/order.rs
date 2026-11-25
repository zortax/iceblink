//! The order tree and output roots.

use iceblink::{EffectId, OrderEntry, OrderId, OrderNode, OutputId, RejectReason};

use crate::naive::commit::check::{creatable, live};
use crate::naive::commit::draft::Draft;
use crate::naive::scene::Order;

pub(super) fn create(
    draft: &mut Draft<'_>,
    id: OrderId,
    node: OrderNode,
) -> Result<(), RejectReason> {
    creatable(&draft.client.orders, id)?;
    if let Some(effect) = node.effect {
        live(&draft.client.effects, effect)?;
    }

    let order = Order {
        node,
        entries: Vec::new(),
    };
    draft.client.orders.create(id, order)
}

pub(super) fn set_effect(
    draft: &mut Draft<'_>,
    id: OrderId,
    effect: Option<EffectId>,
) -> Result<(), RejectReason> {
    live(&draft.client.orders, id)?;
    if let Some(effect) = effect {
        live(&draft.client.effects, effect)?;
    }

    draft
        .client
        .orders
        .get_mut(id)
        .expect("checked above")
        .node
        .effect = effect;
    draft.counters.property_writes += 1;
    Ok(())
}

pub(super) fn splice(
    draft: &mut Draft<'_>,
    node: OrderId,
    at: u32,
    remove: u32,
    insert: &[OrderEntry],
) -> Result<(), RejectReason> {
    live(&draft.client.orders, node)?;
    for entry in insert {
        if let OrderEntry::Node(child) = entry {
            live(&draft.client.orders, *child)?;
        }
    }

    let order = draft.client.orders.get_mut(node).expect("checked above");
    let at = at as usize;
    let end = at
        .checked_add(remove as usize)
        .filter(|end| *end <= order.entries.len())
        .ok_or(RejectReason::SpliceOutOfRange)?;
    order.entries.splice(at..end, insert.iter().copied());
    draft.counters.keys_spliced += remove + insert.len() as u32;
    Ok(())
}

pub(super) fn set_root(
    draft: &mut Draft<'_>,
    output: OutputId,
    root: Option<OrderId>,
) -> Result<(), RejectReason> {
    if let Some(root) = root {
        live(&draft.client.orders, root)?;
    }

    if !draft.outputs.contains(&output) {
        return Err(RejectReason::UnknownOutput);
    }

    match root {
        Some(root) => {
            draft.roots.insert(output, (draft.client_id, root));
        }
        None => {
            draft.roots.remove(&output);
        }
    }

    Ok(())
}
