//! Growing a frame's damage to everything a composite reads that the damage reaches.

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;

/// The share of the output above which the whole output is damaged instead.
const FULL_DAMAGE_SHARE: f64 = 0.5;

/// What one closure did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Expansion {
    /// How many read extents were absorbed.
    pub(crate) absorbed: usize,
    /// How many passes over the read extents ran.
    pub(crate) passes: usize,
    /// Whether the whole output was damaged instead.
    pub(crate) escalated: bool,
}

/// Grows `damage` until every read extent in `sources` that it meets lies inside one of its
/// rectangles, and returns what the closure did.
///
/// Each read extent that meets the damage without lying inside one rectangle is absorbed as one
/// rectangle. Passes repeat until one absorbs nothing. The whole output is damaged when every
/// pass absorbed something, or when the damage covers more than half of the output.
pub(crate) fn expand(
    damage: &mut DamageSet,
    sources: &[DeviceRect],
    output: DeviceRect,
) -> Expansion {
    let mut report = Expansion::default();
    if sources.is_empty() || damage.is_full() {
        return report;
    }

    // The passes that can grow, and the pass that finds nothing left to absorb.
    let bound = sources.len() + 1;
    let area = f64::from(output.size.width.max(0)) * f64::from(output.size.height.max(0));
    for pass in 1..=bound {
        report.passes = pass;

        let mut grew = false;
        for &source in sources {
            if !damage.intersects(source) || contains(damage, source) {
                continue;
            }
            damage.absorb_whole(source);
            report.absorbed += 1;
            grew = true;
        }

        if !grew {
            if area > 0.0
                && let Some(covered) = damage.area()
                && covered as f64 > area * FULL_DAMAGE_SHARE
            {
                damage.set_full();
                report.escalated = true;
            }
            return report;
        }
    }

    damage.set_full();
    report.escalated = true;
    report
}

/// Reports whether one rectangle of the set holds all of `rect`.
fn contains(damage: &DamageSet, rect: DeviceRect) -> bool {
    // The rectangles are disjoint, so a read extent spread across two is held by neither, and the
    // closure keeps absorbing it.
    damage.rects().iter().any(|held| held.contains_rect(rect))
}
