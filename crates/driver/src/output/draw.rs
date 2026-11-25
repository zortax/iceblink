//! Running an output's frames.

use iceblink::{
    Acquired, BufferAge, Built, Compositor, FrameClock, FrameCounters, FrameOutcome, FrameRequest,
    PresentMode, SkipReason,
};

use crate::output::{Drawn, Output};

impl Output {
    /// Runs the due frame of a paced output, which starts at `now_ns`.
    pub(crate) fn draw_paced(
        &mut self,
        compositor: &mut dyn Compositor,
        now_ns: u64,
    ) -> Option<Drawn> {
        // Feedback that came since the frame was planned can move its latch.
        self.read_feedback(now_ns);
        if self.schedule.due().is_some_and(|due| due > now_ns) {
            return None;
        }

        // A frame that latched immediately for a call is measured from its start, since a client
        // that pumps the driver does so only after its own frame.
        let at_once = self.schedule.at_once();
        let latch = if at_once {
            now_ns
        } else {
            self.schedule.due().unwrap_or(now_ns).min(now_ns)
        };
        let budget = self.budget.get(self.schedule.interval_ns);
        let clock = self.schedule.start(now_ns, budget);
        let drawn = self.draw(compositor, clock);
        if drawn.is_none() {
            self.schedule.showed_nothing();
        }

        let within = match &drawn {
            Some(drawn) if drawn.built => self.measure(latch, budget),
            _ => true,
        };
        if let Some(drawn) = &drawn
            && matches!(drawn.outcome, FrameOutcome::Presented(_))
        {
            let present = self
                .renderer
                .presenter()
                .map_or(0, |presenter| presenter.presents());
            // A frame that latched immediately was not given the budget.
            let judged = self.schedule.anchored() && within && !at_once;
            self.display.presented(present, clock.present_ns, judged);
        }
        self.read_feedback(iceblink::now_ns());
        drawn
    }

    /// Takes the cost of a frame drawn now, which latched at `latch`, into the budget, and reports
    /// whether the frame was drawn within `budget_ns`.
    fn measure(&mut self, latch: u64, budget_ns: u64) -> bool {
        // The cost runs from the latch, so a late start counts against the budget.
        let waited = u64::try_from(self.renderer.acquire_block().as_nanos()).unwrap_or(0);
        let cost = iceblink::now_ns().saturating_sub(latch + waited);
        let interval = self.schedule.interval_ns;
        if std::mem::take(&mut self.fresh) {
            self.budget.measure_setup(cost, interval);
        } else {
            self.budget.measure(cost);
        }
        cost <= budget_ns
    }

    /// Runs the frame of a stepped output at `clock`.
    pub(crate) fn draw_stepped(
        &mut self,
        compositor: &mut dyn Compositor,
        clock: FrameClock,
    ) -> Option<Drawn> {
        self.schedule.step(&clock);
        self.draw(compositor, clock)
    }

    /// Runs one frame of the lifecycle at `clock`, then schedules the next one if the output
    /// still changes.
    fn draw(&mut self, compositor: &mut dyn Compositor, clock: FrameClock) -> Option<Drawn> {
        let (age, image) = match self.target_age() {
            Ok(age) => age,
            Err(reason) => {
                // Nothing was built, so the compositor keeps the frame's damage for the retry.
                let outcome = FrameOutcome::Skipped(reason);
                self.finish(outcome, clock.now_ns, None);
                return Some(Drawn {
                    frame: self.frame.id,
                    counters: FrameCounters::default(),
                    outcome,
                    built: false,
                });
            }
        };

        let caps = self.renderer.capabilities();
        let request = FrameRequest {
            caps: &caps,
            age,
            clock,
            subpixel: self.subpixel,
        };
        match compositor.build_frame(self.id, &request, &mut self.frame) {
            Built::Frame => Some(self.draw_built(compositor, clock)),
            Built::Idle { wake_ns } => self.idle(image, wake_ns, clock),
        }
    }

    /// Returns the age of what the next frame is composed into, and the image acquired for it in
    /// buffer-age mode.
    ///
    /// Returns the reason when no image could be acquired.
    fn target_age(&mut self) -> Result<(BufferAge, Option<u32>), SkipReason> {
        let buffer_age = self
            .renderer
            .presenter()
            .is_some_and(|presenter| presenter.capabilities().mode == PresentMode::BufferAge);
        if buffer_age {
            self.acquire().map(|(age, image)| (age, Some(image)))
        } else {
            Ok((self.renderer.target_age(), None))
        }
    }

    /// Draws the frame just built at `clock`, reports its outcome to `compositor`, and schedules
    /// the next frame.
    fn draw_built(&mut self, compositor: &mut dyn Compositor, clock: FrameClock) -> Drawn {
        self.frames_built += 1;
        let counters = compositor.counters().frame;
        if let Some(observer) = &mut self.observer {
            observer(&self.frame);
        }
        let outcome = self.renderer.draw(&self.frame);
        compositor.frame_done(self.id, self.frame.id, &outcome);

        self.unpresented = matches!(outcome, FrameOutcome::Skipped(_)) && outcome.retires_damage();
        if outcome == FrameOutcome::Skipped(SkipReason::Held) {
            self.state.observe(outcome);
            self.retry_held(clock.now_ns, clock.present_ns);
        } else {
            self.finish(outcome, clock.now_ns, compositor.next_wake(self.id));
        }
        Drawn {
            frame: self.frame.id,
            counters,
            outcome,
            built: true,
        }
    }

    /// Releases `image` after a build at `clock` found nothing to draw, presents a frame that
    /// waits to be presented, and schedules the next frame for the compositor's next change at
    /// `wake_ns`.
    ///
    /// Returns the frame presented, if any.
    fn idle(
        &mut self,
        image: Option<u32>,
        wake_ns: Option<u64>,
        clock: FrameClock,
    ) -> Option<Drawn> {
        if let Some(image) = image
            && let Some(presenter) = self.renderer.presenter()
        {
            presenter.release(image);
        }
        if !self.unpresented {
            self.wake_at(wake_ns, clock.now_ns);
            return None;
        }

        let outcome = self.renderer.present_held();
        // The renderer keeps the frame until it is presented or nothing is left to show.
        self.unpresented = !matches!(
            outcome,
            FrameOutcome::Presented(_) | FrameOutcome::Skipped(SkipReason::Undamaged)
        );
        if outcome == FrameOutcome::Skipped(SkipReason::Held) {
            self.state.observe(outcome);
            self.retry_held(clock.now_ns, self.frame.clock.present_ns);
            self.wake_at(wake_ns, clock.now_ns);
        } else {
            self.finish(outcome, clock.now_ns, wake_ns);
        }
        matches!(outcome, FrameOutcome::Presented(_)).then_some(Drawn {
            frame: self.frame.id,
            counters: FrameCounters::default(),
            outcome,
            built: false,
        })
    }

    /// Acquires the image a buffer-age frame is composed into.
    fn acquire(&mut self) -> Result<(BufferAge, u32), SkipReason> {
        let Some(presenter) = self.renderer.presenter() else {
            return Err(SkipReason::Unconfigured);
        };
        match presenter.acquire() {
            Acquired::Image { image, age } => Ok((age, image)),
            Acquired::Skipped(reason) => Err(reason),
        }
    }

    /// Schedules the frame after one that ended with `outcome`, for which the compositor reported
    /// its next change at `wake`.
    ///
    /// The schedule depends on the outcome:
    ///
    /// - If the output waits for the platform or a replacement device, no frame is wanted.
    /// - If the outcome asks for another frame, or a held frame waits to be presented, the next
    ///   frame is wanted.
    /// - Otherwise the frame that shows the compositor's next change is wanted, if any.
    fn finish(&mut self, outcome: FrameOutcome, now_ns: u64, wake: Option<u64>) {
        self.state.observe(outcome);
        match outcome {
            FrameOutcome::Skipped(SkipReason::Occluded | SkipReason::DeviceUnavailable) => {
                self.schedule.park();
            }
            // A retry, or a held frame the next build presents.
            _ if outcome.wants_another_frame() || self.unpresented => self.want(now_ns),
            _ => self.wake_at(wake, now_ns),
        }
    }
}
