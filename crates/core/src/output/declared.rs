//! Derived damage checked against the damage a commit declared.

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;

/// The damage a commit derives for an output that it declared damage for.
#[derive(Debug)]
pub(crate) struct DeclaredCheck {
    declared: DamageSet,
    pub(crate) derived: DamageSet,
    /// Whether a derived rectangle reached outside the declaration.
    pub(crate) undeclared: bool,
}

impl DeclaredCheck {
    /// Starts checking derived damage against `declared`.
    pub(crate) const fn new(declared: DamageSet) -> Self {
        Self {
            declared,
            derived: DamageSet::new(),
            undeclared: false,
        }
    }

    /// Adds `rect` to the derived damage, and notes whether the declaration holds all of it.
    pub(crate) fn absorb(&mut self, rect: DeviceRect) {
        // Checked before the set merges it with its neighbours, whose union can reach past the
        // declaration where no change does.
        self.undeclared |= !covers(&self.declared, rect);
        self.derived.absorb(rect);
    }

    /// Makes the derived damage the whole output.
    pub(crate) fn set_full(&mut self) {
        self.undeclared |= !self.declared.is_full();
        self.derived.set_full();
    }
}

/// Reports whether every pixel of `rect` lies in `declared`.
fn covers(declared: &DamageSet, rect: DeviceRect) -> bool {
    if declared.is_full() {
        return true;
    }

    // The declared rectangles are disjoint, so their overlaps with a rectangle add up to its area
    // exactly when they cover it.
    let covered: i64 = declared
        .rects()
        .iter()
        .filter_map(|held| held.intersection(rect))
        .map(area)
        .sum();
    covered == area(rect)
}

fn area(rect: DeviceRect) -> i64 {
    i64::from(rect.size.width.max(0)) * i64::from(rect.size.height.max(0))
}
