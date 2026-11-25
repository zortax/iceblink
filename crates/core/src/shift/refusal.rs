//! Why a frame answered a scroll offset change with damage, and counts of each reason.

/// The condition of the shift rule a scroll offset change failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The scroll node belongs to a client the output does not draw.
    Foreign,
    /// The moving content did not move by one translation of whole pixels.
    NotRigid,
    /// The moving content did not move.
    Unmoved,
    /// The port does not map to a rectangle of whole pixels on the output.
    Port,
    /// The ink of the moving content leaves the port before or after the change, or a clip link
    /// that stays still cuts it inside the port.
    Uncontained,
    /// Moving content with ink is drawn in a group whose filters spread it.
    Spread,
    /// The region meets the region of a shift the frame already makes.
    Overlap,
    /// A reading composite reads in the region.
    Read,
    /// The output's order tree does not draw the moving content.
    NotDrawn,
    /// What the content is drawn over in the region is not one opaque colour that nothing draws
    /// across.
    NotBacked,
    /// Something outside the content draws in the region after it, or an isolating group around
    /// it or beside it meets the region.
    Overdrawn,
}

impl Refusal {
    /// Every refusal, in the order of their discriminants.
    const ALL: [Self; 11] = [
        Self::Foreign,
        Self::NotRigid,
        Self::Unmoved,
        Self::Port,
        Self::Uncontained,
        Self::Spread,
        Self::Overlap,
        Self::Read,
        Self::NotDrawn,
        Self::NotBacked,
        Self::Overdrawn,
    ];

    /// Returns the stable name `extra_stats` reports the count under.
    fn name(self) -> &'static str {
        match self {
            Self::Foreign => "shift.refused.foreign",
            Self::NotRigid => "shift.refused.not_rigid",
            Self::Unmoved => "shift.refused.unmoved",
            Self::Port => "shift.refused.port",
            Self::Uncontained => "shift.refused.uncontained",
            Self::Spread => "shift.refused.spread",
            Self::Overlap => "shift.refused.overlap",
            Self::Read => "shift.refused.read",
            Self::NotDrawn => "shift.refused.not_drawn",
            Self::NotBacked => "shift.refused.not_backed",
            Self::Overdrawn => "shift.refused.overdrawn",
        }
    }
}

/// Counts of the refused scroll offset changes by reason, since the compositor was created.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Refusals {
    counts: [u64; Refusal::ALL.len()],
}

impl Refusals {
    pub(crate) fn add(&mut self, refusal: Refusal) {
        self.counts[refusal as usize] += 1;
    }

    /// Calls `out` with every count by its stable name.
    pub(crate) fn report(&self, out: &mut dyn FnMut(&'static str, u64)) {
        for refusal in Refusal::ALL {
            out(refusal.name(), self.counts[refusal as usize]);
        }
    }
}
