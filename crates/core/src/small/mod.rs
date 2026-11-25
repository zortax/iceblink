//! Maps and sets for the few keys a small commit touches, which allocate only once they grow,
//! and a set of slots for the many a large commit touches.

mod map;
mod slots;

pub(crate) use self::map::{SmallMap, SmallSet};
pub(crate) use self::slots::SlotSet;
