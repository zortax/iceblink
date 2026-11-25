//! Presenting a composed frame in kept-target mode: acquiring an image, copying its debt from the
//! kept target and presenting it.

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{Acquired, FrameOutcome, FrameStats, PresentResult, SkipReason};

use crate::renderer::CpuRenderer;

impl CpuRenderer {
    /// Presents the last composed frame with the damage composed since the last present.
    pub(super) fn present_composed(&mut self, mut stats: FrameStats) -> FrameOutcome {
        let damage = self.unpresented.unwrap_or_default();

        let asked = std::time::Instant::now();
        let acquired = self.presenter.acquire();
        self.acquire_block = asked.elapsed();
        let image = match acquired {
            Acquired::Image { image, .. } => image,
            Acquired::Skipped(reason) => return FrameOutcome::Skipped(retiring(reason)),
        };

        stats.present_copy_px = self.copy_debt(image);
        let extent = self.presenter.extent();
        let result = self.presenter.present(image, &damage);
        self.finish_present(result);
        stats.present_damage_px = area_of(&damage, extent);
        of_present(result, stats)
    }

    /// Records the result of a present.
    pub(super) fn finish_present(&mut self, result: PresentResult) {
        if result == PresentResult::Presented {
            self.unpresented = None;
        }
    }

    // Copies the debt of `image` from the kept target, and returns how many pixels it copied.
    fn copy_debt(&mut self, image: u32) -> u64 {
        let debt = *self.presenter.debt(image);
        let Some(pixmap) = self.presenter.image_mut(image) else {
            return 0;
        };
        let kept = &self.kept;
        let width = pixmap.width().min(kept.width()) as i32;
        let height = pixmap.height().min(kept.height()) as i32;
        let whole = Rect::new(Point::new(0, 0), Size::new(width, height));
        let rects: Vec<_> = if debt.is_full() {
            vec![whole]
        } else {
            debt.rects()
                .iter()
                .filter_map(|rect| rect.intersection(whole))
                .collect()
        };
        let (from_width, to_width) = (kept.width() as usize, pixmap.width() as usize);
        let source = kept.texels();
        let destination = pixmap.texels_mut();
        let mut copied = 0;
        for rect in rects {
            let columns = rect.width() as usize;
            for y in rect.top()..rect.bottom() {
                let from = y as usize * from_width + rect.left() as usize;
                let to = y as usize * to_width + rect.left() as usize;
                destination[to..to + columns].copy_from_slice(&source[from..from + columns]);
            }
            copied += columns as u64 * rect.height() as u64;
        }
        copied
    }
}

/// Returns the outcome of a frame that was composed and then presented with `result`.
pub(super) fn of_present(result: PresentResult, stats: FrameStats) -> FrameOutcome {
    match result {
        PresentResult::Presented => FrameOutcome::Presented(stats),
        PresentResult::Held => FrameOutcome::Skipped(SkipReason::Held),
        PresentResult::Skipped(reason) => FrameOutcome::Skipped(retiring(reason)),
    }
}

/// Returns the skip reason of a frame that was already composed, which retires its damage.
///
/// The reasons that keep damage, `Unconfigured`, `Unacquired` and `DeviceUnavailable`, become
/// `Timeout`.
pub(super) fn retiring(reason: SkipReason) -> SkipReason {
    match reason {
        SkipReason::Unconfigured | SkipReason::Unacquired | SkipReason::DeviceUnavailable => {
            SkipReason::Timeout
        }
        other => other,
    }
}

/// Returns how many pixels of an image of `extent` lie in `damage`.
pub(super) fn area_of(damage: &DamageSet, extent: [u32; 2]) -> u64 {
    let whole = Rect::new(
        Point::new(0, 0),
        Size::new(extent[0] as i32, extent[1] as i32),
    );
    if damage.is_full() {
        return u64::from(extent[0]) * u64::from(extent[1]);
    }
    damage
        .rects()
        .iter()
        .filter_map(|rect| rect.intersection(whole))
        .map(|rect| rect.width() as u64 * rect.height() as u64)
        .sum()
}
