//! Presenting composed frames: holding them behind the present gate, and in kept-target mode
//! acquiring an image, copying its debt and presenting it.

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{Acquired, FrameOutcome, FrameStats, PresentResult, SkipReason};

use crate::frame::damage;
use crate::renderer::{WgpuRenderer, outcome};

impl WgpuRenderer {
    /// Holds the composed frame while it may not be presented, and presents it otherwise.
    ///
    /// The frame's `encoder`, if any, is submitted either way.
    pub(super) fn gated_present(
        &mut self,
        stats: FrameStats,
        encoder: Option<wgpu::CommandEncoder>,
    ) -> FrameOutcome {
        if !self.may_present() {
            if let Some(encoder) = encoder {
                self.submit_frame(encoder);
            }
            self.held = true;
            self.latency.mark("sub.held");
            return FrameOutcome::Skipped(SkipReason::Held);
        }
        self.present_composed(stats, encoder)
    }

    /// Reports whether the platform takes an image now and the display has room for it.
    // The second condition keeps a present from queueing behind those the display holds, which
    // would show every later frame a refresh late.
    pub(super) fn may_present(&self) -> bool {
        self.present_gate.as_ref().is_none_or(|gate| gate()) && !self.presenter.display_full()
    }

    /// Acquires an image, copies its debt from the kept target, and presents it with the damage
    /// composed since the last present.
    ///
    /// The copy is recorded into `encoder` when there is one.
    // Recording into the frame's own encoder makes the frame submit once.
    pub(super) fn present_composed(
        &mut self,
        mut stats: FrameStats,
        encoder: Option<wgpu::CommandEncoder>,
    ) -> FrameOutcome {
        let damage = self.unpresented.unwrap_or_default();

        self.latency.mark("acq.in");
        let asked = web_time::Instant::now();
        let acquired = self.presenter.acquire();
        self.acquire_block = asked.elapsed();
        self.gpu.reclaim();
        let image = match acquired {
            Acquired::Image { image, .. } => image,
            Acquired::Skipped(reason) => {
                if let Some(encoder) = encoder {
                    self.submit_frame(encoder);
                }
                self.latency.mark("acq.skipped");
                return FrameOutcome::Skipped(outcome::retiring(reason));
            }
        };
        self.latency.mark("acq.out");

        let extent = self.presenter.extent();
        let scissors = self.debt_scissors(self.presenter.debt(image), extent);
        let view = self.presenter.image_view(image).cloned();
        let mut encoder = encoder.unwrap_or_else(|| self.frame_encoder());
        if let Some(view) = &view {
            let (draws, copied) = self.blit(&mut encoder, view, &scissors);
            stats.draw_calls += draws;
            stats.present_copy_px = copied;
        }
        self.submit_frame(encoder);

        let used = self.kept.used().size;
        self.presenter.check_image(
            image,
            self.kept.texture(),
            [used.width as u32, used.height as u32],
        );
        if let (Some(notify), Some(target)) = (&self.pre_present, self.target) {
            notify(target);
        }
        let (refresh, interval) = self.refreshes;
        self.presenter.follow_refreshes(refresh, interval);
        let result = self.presenter.present(image, &damage);
        self.finish_present(image, result, &damage);
        stats.present_damage_px = area_of(&damage, extent);
        self.latency.mark("pres.out");
        outcome::of_present(result, stats)
    }

    /// Records `image` and `damage` as the last present if `result` says the image was presented.
    pub(super) fn finish_present(&mut self, image: u32, result: PresentResult, damage: &DamageSet) {
        if result == PresentResult::Presented {
            self.unpresented = None;
            self.presented = Some(image);
            self.presented_damage = *damage;
        }
    }
}

/// Returns the pixels `damage` covers in an image of `extent`.
fn area_of(damage: &DamageSet, extent: [u32; 2]) -> u64 {
    let whole = Rect::new(
        Point::new(0, 0),
        Size::new(extent[0] as i32, extent[1] as i32),
    );
    damage::rects(damage, whole)
        .iter()
        .map(|rect| damage::area(*rect))
        .sum()
}
