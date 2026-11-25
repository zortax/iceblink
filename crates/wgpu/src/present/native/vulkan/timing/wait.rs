//! The waiter: it waits for each submitted present in turn and notes when it was shown.

use std::thread;
use std::time::Duration;

use ash::vk;
use web_time::Instant;

use super::{Shared, lock};

/// How long a present is waited for before it is taken to never be shown on the display.
// An occluded window's presents are never shown.
const PATIENCE: Duration = Duration::from_millis(250);

/// How long before a refresh the checks for a present that may reach it begin.
// Presents a key press asks for were shown up to 200 µs before the refresh they were planned for.
const LEAD_NS: u64 = 200_000;

/// How long after a refresh the checks go on at the full rate.
const TRAIL_NS: u64 = 1_000_000;

/// How long the waiter sleeps between two checks around a refresh.
// The present wait of NVIDIA 610 on an RTX 3080 Ti wakes about every 13 µs while it waits, so a
// wait with a timeout costs a wakeup each time. A check without one costs about two.
const CHECK_NS: u64 = 25_000;

/// How long the waiter sleeps between two checks while it does not know which refresh a present
/// can reach.
const UNPLANNED_CHECK_NS: u64 = 100_000;

/// The next present to wait for.
struct Next {
    swapchain: vk::SwapchainKHR,
    id: u64,
    /// The generation of `swapchain`.
    generation: u64,
    /// When the present was submitted, on the frame clock.
    submitted_ns: u64,
    /// Whether every present before it had been shown or given up on when it was submitted.
    alone: bool,
    /// The first refresh the present can reach and the interval, while the display times agree
    /// with the refreshes.
    first: Option<(u64, u64)>,
}

/// How a wait for a present ended.
enum Waited {
    /// The present was shown, and the waiter saw when, unless the first check after a sleep found
    /// it shown already.
    Shown { at_ns: u64, seen: bool },
    /// The patience ran out, the wait failed, the swap chain was replaced or the chain stopped.
    Dropped,
}

impl Shared {
    /// Waits for every submitted present in turn, until the chain stops.
    pub(super) fn wait_presents(&self, wait: &ash::khr::present_wait::Device) {
        while let Some(next) = self.next_present() {
            let waited = self.wait_one(wait, next.swapchain, next.id, next.generation, next.first);
            self.note(&next, waited);
        }
    }

    /// Waits until a present is submitted that was not waited for, and returns it, or `None`
    /// once the chain stops.
    fn next_present(&self) -> Option<Next> {
        let mut state = self
            .changed
            .wait_while(lock(&self.state), |state| {
                !state.stop
                    && (state.swapchain == vk::SwapchainKHR::null()
                        || state.waited >= state.submitted)
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.stop {
            return None;
        }

        let id = state.waited + 1;
        while state.queued.front().is_some_and(|queued| queued.id < id) {
            state.queued.pop_front();
        }
        let queued = state.queued.front().copied();
        let submitted = queued.map_or_else(iceblink::now_ns, |queued| queued.at_ns);
        Some(Next {
            swapchain: state.swapchain,
            id,
            generation: state.generation,
            submitted_ns: submitted,
            alone: queued.is_some_and(|queued| queued.alone),
            first: state.refreshes.first_after(submitted),
        })
    }

    /// Notes how the wait for the present `next` ended, unless its swap chain was replaced since.
    fn note(&self, next: &Next, waited: Waited) {
        let mut state = lock(&self.state);
        if state.generation != next.generation {
            return;
        }
        state.waited = next.id;
        let at_ns = match waited {
            Waited::Shown { at_ns, seen } => {
                state.refreshes.observe(at_ns, seen);
                // A present that queued behind others measures them as well as the display.
                if next.alone {
                    state.depth.observe(next.submitted_ns, at_ns);
                }
                at_ns
            }
            // A present given up on is reported when it was given up on, so every present
            // submitted is reported once.
            Waited::Dropped => iceblink::now_ns(),
        };
        let present = state.before + next.id;
        state.presented.push((present, at_ns));
    }

    /// Waits until present `id` of `swapchain` is shown on the display.
    ///
    /// The waiter checks for the present without waiting, and sleeps between two checks. With
    /// `first`, the first refresh the present can reach and the interval, it sleeps until shortly
    /// before each refresh and checks from then until half an interval after it, ever less often
    /// once a millisecond has passed. Without, it checks throughout, less often.
    fn wait_one(
        &self,
        wait: &ash::khr::present_wait::Device,
        swapchain: vk::SwapchainKHR,
        id: u64,
        generation: u64,
        first: Option<(u64, u64)>,
    ) -> Waited {
        let started = Instant::now();
        let mut window = first;
        let mut woke = false;
        let mut late_gap = UNPLANNED_CHECK_NS;
        loop {
            if let Some((refresh, interval)) = &mut window {
                let now = iceblink::now_ns();
                // The display server may report a present well after the refresh that showed it.
                while now > *refresh + *interval / 2 {
                    *refresh += *interval;
                    late_gap = UNPLANNED_CHECK_NS;
                }
                if now + LEAD_NS < *refresh {
                    if !self.sleep_until(*refresh - LEAD_NS, generation) {
                        return Waited::Dropped;
                    }
                    woke = true;
                }
            }
            if self.wanted.load(std::sync::atomic::Ordering::Acquire) > 0 {
                thread::sleep(Duration::from_micros(50));
                continue;
            }
            let held = lock(&self.swapchain);
            {
                let state = lock(&self.state);
                if state.stop || state.generation != generation {
                    return Waited::Dropped;
                }
            }
            // SAFETY: the swap chain is live, since a replacement or a drop takes the swap chain
            // lock held here and changes the generation checked above. The id was submitted.
            let checked = unsafe { wait.wait_for_present(swapchain, id, 0) };
            drop(held);
            match checked {
                Ok(()) => {
                    let at_ns = iceblink::now_ns();
                    // Shown before the first check after a sleep, at a time nobody saw.
                    let seen = !woke;
                    return Waited::Shown { at_ns, seen };
                }
                Err(vk::Result::TIMEOUT) if started.elapsed() < PATIENCE => {
                    woke = false;
                    let gap = match window {
                        Some((refresh, _)) if iceblink::now_ns() > refresh + TRAIL_NS => {
                            late_gap *= 2;
                            late_gap / 2
                        }
                        Some(_) => CHECK_NS,
                        None => UNPLANNED_CHECK_NS,
                    };
                    thread::sleep(Duration::from_nanos(gap));
                }
                Err(_) => return Waited::Dropped,
            }
        }
    }

    /// Sleeps until `wake_ns` on the frame clock, and reports whether the swap chain of
    /// `generation` is still current and the chain still runs.
    fn sleep_until(&self, wake_ns: u64, generation: u64) -> bool {
        let wait = Duration::from_nanos(wake_ns.saturating_sub(iceblink::now_ns()));
        let (state, _) = self
            .changed
            .wait_timeout_while(lock(&self.state), wait, |state| {
                !state.stop && state.generation == generation
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !state.stop && state.generation == generation
    }
}
