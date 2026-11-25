//! Readings of the rule that starts a new vector pass.

use iceblink::DeviceRect;

/// A drawn non-path instance that came while a pass was accumulating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Intervening {
    /// Its device ink.
    pub bounds: DeviceRect,
    /// How many of the pass's paths came before it.
    pub accumulated: usize,
}

/// How the planner decides that an intervening instance overlaps the paths accumulated in a pass,
/// which starts a new pass at the next path.
///
/// The readings agree when a pass's paths form one blob, and differ on a grid of separate
/// drawings.
// On a dashboard of twenty charts with each legend drawn under its chart, the order-blind box, the
// box and the per-ink readings plan twenty, four and one pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overlap {
    /// Tests the instance against the ink of each path that came before it.
    // Exact: a pass draws nothing outside its paths' inks, and compositing transparent texels
    // changes no pixel.
    #[default]
    PerItemInk,
    /// Tests the instance against the bounding box of the paths that came before it.
    BoundingBox,
    /// Tests every instance since the pass began against the bounding box of the whole pass, the
    /// next path included, whenever they came.
    BoundingBoxOrderBlind,
    /// Never starts a pass for an intervening instance.
    ///
    /// The plan is then correct only for passes composited one path at a time.
    Never,
}

impl Overlap {
    /// Reports whether admitting the path of ink `candidate` would trap one of `intervening` under
    /// the pass's composite.
    ///
    /// `accumulated` holds the inks of the paths already in the pass, in paint order.
    pub fn splits(
        self,
        accumulated: &[DeviceRect],
        candidate: DeviceRect,
        intervening: &[Intervening],
    ) -> bool {
        match self {
            Self::BoundingBoxOrderBlind => accumulated
                .iter()
                .copied()
                .chain(core::iter::once(candidate))
                .reduce(DeviceRect::union)
                .is_some_and(|whole| {
                    intervening
                        .iter()
                        .any(|instance| instance.bounds.intersects(whole))
                }),
            _ => intervening
                .iter()
                .any(|instance| self.caught(accumulated, instance)),
        }
    }

    /// Reports whether the reading decides a split without the next path's ink.
    ///
    /// Every reading but [`Overlap::BoundingBoxOrderBlind`] does.
    pub fn ignores_candidate(self) -> bool {
        self != Self::BoundingBoxOrderBlind
    }

    /// Reports whether `instance` alone makes [`Overlap::splits`] true for every next path.
    ///
    /// The answer is always `false` for [`Overlap::BoundingBoxOrderBlind`] and [`Overlap::Never`].
    pub fn caught(self, accumulated: &[DeviceRect], instance: &Intervening) -> bool {
        let earlier = &accumulated[..instance.accumulated.min(accumulated.len())];
        match self {
            Self::Never | Self::BoundingBoxOrderBlind => false,
            Self::BoundingBox => earlier
                .iter()
                .copied()
                .reduce(DeviceRect::union)
                .is_some_and(|bounds| instance.bounds.intersects(bounds)),
            Self::PerItemInk => earlier.iter().any(|ink| instance.bounds.intersects(*ink)),
        }
    }
}
