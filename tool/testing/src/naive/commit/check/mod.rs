//! Checks of the values a transaction carries.

pub(crate) mod finite;
pub(crate) mod items;
pub(crate) mod nodes;

use iceblink::{Id, RejectReason};

use crate::naive::scene::Table;

/// Fails with `StaleId` unless `id` refers to a live object of `table` with its generation.
pub(crate) fn live<T, K>(table: &Table<T>, id: Id<K>) -> Result<(), RejectReason> {
    if table.is_live(id) {
        Ok(())
    } else {
        Err(RejectReason::StaleId)
    }
}

/// Fails with `StaleId` unless a create may use `id`.
pub(crate) fn creatable<T, K>(table: &Table<T>, id: Id<K>) -> Result<(), RejectReason> {
    if table.can_create(id) {
        Ok(())
    } else {
        Err(RejectReason::StaleId)
    }
}
