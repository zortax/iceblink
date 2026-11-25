//! Orders of instances among themselves, for blocks.

use iceblink::DeviceRect;

use crate::plan::order::{BoundsTree, DrawOrder};

/// Writes into `out` each ink's order among `inks`, counting from one.
///
/// An ink takes one more than the highest order of the earlier inks it overlaps, which is the order
/// a [`BoundsTree`] gives it when `inks` are inserted alone into an empty tree. These are the local
/// orders of an [`Emission`](crate::plan::Emission) block, and stay valid while the inks move
/// together by whole pixels. `tree` is scratch space and is cleared.
// Inks whose left edges never step back take a scan that stops where no earlier ink can reach,
// which keeps a line of glyphs linear.
pub fn local_orders(inks: &[DeviceRect], tree: &mut BoundsTree, out: &mut Vec<DrawOrder>) {
    out.clear();
    if scan(inks, out) {
        return;
    }

    out.clear();
    tree.clear();
    out.extend(inks.iter().map(|ink| tree.insert(*ink)));
    tree.clear();
}

/// Orders a left-to-right sequence into `out`, and reports `false` when an ink steps back.
fn scan(inks: &[DeviceRect], out: &mut Vec<DrawOrder>) -> bool {
    let mut widest = 0i64;
    for (at, rect) in inks.iter().enumerate() {
        if at > 0 && rect.left() < inks[at - 1].left() {
            return false;
        }

        let mut order = 1;
        for earlier in (0..at).rev() {
            let other = inks[earlier];
            if i64::from(other.left()) + widest < i64::from(rect.left()) {
                break;
            }
            if other.intersects(*rect) {
                order = order.max(out[earlier] + 1);
            }
        }
        widest = widest.max(i64::from(rect.size.width));
        out.push(order);
    }
    true
}
