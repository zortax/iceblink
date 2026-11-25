//! Whether one composite can carry a whole pass.

use iceblink::DeviceRect;

use crate::plan::order::DrawOrder;
use crate::plan::pass::cells::InkCells;
use crate::plan::pass::overlap::Intervening;

/// Reports whether compositing a pass as one draw at `composite` would cover an intervening
/// instance that belongs above one of its paths.
///
/// `inks` and `orders` describe the pass's paths in paint order. An instance that came after a
/// path and overlaps its ink belongs above that path, so it is trapped when that path's order is
/// below `composite`.
pub(crate) fn traps(
    inks: &[DeviceRect],
    cells: &InkCells,
    orders: &[DrawOrder],
    intervening: &[Intervening],
    composite: DrawOrder,
) -> bool {
    intervening.iter().any(|instance| {
        let earlier = instance.accumulated.min(inks.len());
        cells.meets(instance.bounds, inks, |index| {
            index < earlier && orders[index] < composite
        })
    })
}
