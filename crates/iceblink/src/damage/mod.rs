//! Sets of damage rectangles, and visited sets that are emptied in constant time.
//!
//! | Type | Holds |
//! |---|---|
//! | [`DamageSet`] | The rectangles of a surface that must be redrawn |
//! | [`EpochBitset`] | The indices a walk has visited, emptied in constant time |
//!
//! ```
//! use iceblink::damage::DamageSet;
//! use iceblink::geom::{Point, Rect, Size};
//!
//! let mut damage = DamageSet::<4>::new();
//! damage.absorb(Rect::new(Point::new(20, 20), Size::new(80, 24)));
//! assert!(damage.intersects(Rect::new(Point::new(0, 0), Size::new(32, 32))));
//! ```

pub mod damage_set;
pub mod epoch_bitset;

pub use crate::damage::damage_set::{
    COALESCE_WASTE_SHARE, DamageSet, MAX_DAMAGE, full_damage_forced,
};
pub use crate::damage::epoch_bitset::EpochBitset;
