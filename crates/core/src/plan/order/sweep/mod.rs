//! The check that instances at equal draw order do not overlap.
//!
//! An order class is swept by left edge, keeping the rectangles whose right edge is still ahead,
//! so a class of `k` instances costs `k log k`.

mod refusal;

use iceblink::DeviceRect;

use crate::plan::batch::DrawKind;
use crate::plan::order::DrawOrder;

pub use self::refusal::TooManyAtOneOrder;

/// The default cap on the size of one order class.
pub const DEFAULT_MAX: usize = 20_000;

/// Two instances at one draw order that overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderOverlap {
    /// The order both were given.
    pub order: DrawOrder,
    /// The kind of the one earlier in the sweep.
    pub first: DrawKind,
    /// The kind of the other.
    pub second: DrawKind,
    /// The pixels both cover.
    pub shared: DeviceRect,
}

/// One instance as the sweep sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ordered {
    /// The order it was given.
    pub order: DrawOrder,
    /// Its kind.
    pub kind: DrawKind,
    /// Its device ink.
    pub ink: DeviceRect,
}

/// Returns every pair of instances at equal order whose inks overlap.
///
/// Instances with empty ink overlap nothing.
///
/// # Errors
///
/// Fails with [`TooManyAtOneOrder`] when one order class holds more than `max` instances.
///
/// ```
/// use iceblink_core::plan::DrawKind;
/// use iceblink_core::plan::order::sweep::{Ordered, overlaps};
/// use iceblink::DeviceRect;
/// use iceblink::geom::{Point, Size};
///
/// let at = |x: i32| DeviceRect::new(Point::new(x, 0), Size::new(10, 10));
/// let one = |x| Ordered { order: 1, kind: DrawKind::Quad, ink: at(x) };
///
/// assert!(overlaps(&[one(0), one(40)], 16)?.is_empty());
/// assert_eq!(overlaps(&[one(0), one(5)], 16)?.len(), 1);
/// # Ok::<(), iceblink_core::plan::order::sweep::TooManyAtOneOrder>(())
/// ```
pub fn overlaps(instances: &[Ordered], max: usize) -> Result<Vec<OrderOverlap>, TooManyAtOneOrder> {
    let mut by_order: Vec<&Ordered> = instances
        .iter()
        .filter(|held| !held.ink.is_empty())
        .collect();
    by_order.sort_by(|one, two| {
        one.order
            .cmp(&two.order)
            .then(one.ink.left().cmp(&two.ink.left()))
    });

    let mut found = Vec::new();
    let mut start = 0;
    while start < by_order.len() {
        let order = by_order[start].order;
        let mut end = start;
        while end < by_order.len() && by_order[end].order == order {
            end += 1;
        }

        let class = &by_order[start..end];
        if class.len() > max {
            return Err(TooManyAtOneOrder {
                order,
                held: class.len(),
                max,
            });
        }
        sweep(class, &mut found);
        start = end;
    }
    Ok(found)
}

/// Appends every overlapping pair of one order class, sorted by left edge, to `found`.
fn sweep(class: &[&Ordered], found: &mut Vec<OrderOverlap>) {
    let mut active: Vec<&Ordered> = Vec::new();
    for held in class {
        let edge = held.ink.left();
        // Sorted by left edge, so a rectangle ending before this edge meets nothing later.
        active.retain(|open: &&Ordered| open.ink.right() > edge);
        for open in &active {
            if let Some(shared) = open.ink.intersection(held.ink) {
                found.push(OrderOverlap {
                    order: held.order,
                    first: open.kind,
                    second: held.kind,
                    shared,
                });
            }
        }
        active.push(held);
    }
}
