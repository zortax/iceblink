//! Timing presents: a thread that waits for each present to be shown on the display and notes
//! when.
//!
//! The thread checks for a present without waiting and sleeps between checks. Once the display
//! times agree with the refreshes frames are planned on, it sleeps between refreshes and checks
//! only around each refresh a present can reach.

mod depth;
mod refreshes;
mod wait;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use ash::vk;

use self::depth::Depth;
use self::refreshes::Refreshes;

/// The presents of one chain and when they were shown on the display.
pub(super) struct PresentTiming {
    shared: Arc<Shared>,
    waiter: Option<JoinHandle<()>>,
}

/// The state the chain and the waiter share.
struct Shared {
    /// The lock every call that uses the swap chain holds.
    // The swap chain is externally synchronized, and the waiter calls `vkWaitForPresentKHR` on it.
    swapchain: Mutex<()>,
    /// How many calls wait to take the swap chain, which the waiter lets go first.
    wanted: AtomicUsize,
    state: Mutex<State>,
    /// Notified when a present is submitted or the chain stops.
    changed: Condvar,
}

/// A submitted present the waiter has not waited for yet.
#[derive(Clone, Copy)]
struct Queued {
    id: u64,
    /// When the present was submitted, on the frame clock.
    at_ns: u64,
    /// Whether every present before it had been shown or given up on when it was submitted.
    alone: bool,
}

/// The presents of the current swap chain and the waiter's progress through them.
#[derive(Default)]
struct State {
    swapchain: vk::SwapchainKHR,
    /// Counts the swap chains made, so a wait on a replaced one is dropped.
    generation: u64,
    /// The presents of the swap chains before this one, which number this one's presents on.
    before: u64,
    /// The id of the newest present submitted.
    submitted: u64,
    /// The id of the newest present waited for.
    waited: u64,
    /// The presents not waited for yet, oldest first.
    queued: VecDeque<Queued>,
    /// The refreshes frames are planned on.
    refreshes: Refreshes,
    depth: Depth,
    /// The numbers of presents and when they were shown on the display, on the frame clock,
    /// oldest first.
    presented: Vec<(u64, u64)>,
    /// Whether the waiter stops.
    stop: bool,
}

impl PresentTiming {
    /// Starts a waiter that waits for presents through `wait`, which belongs to a device with
    /// present ids and present waits.
    ///
    /// Returns `None` if the waiter thread cannot be started.
    pub(super) fn new(wait: ash::khr::present_wait::Device) -> Option<Self> {
        let shared = Arc::new(Shared {
            swapchain: Mutex::new(()),
            wanted: AtomicUsize::new(0),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let waiter = thread::Builder::new()
            .name("present-timing".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || shared.wait_presents(&wait)
            })
            .ok()?;
        Some(Self {
            shared,
            waiter: Some(waiter),
        })
    }

    /// Locks the swap chain against the waiter, for a call that uses it.
    pub(super) fn hold(&self) -> MutexGuard<'_, ()> {
        self.shared.wanted.fetch_add(1, Ordering::AcqRel);
        let held = lock(&self.shared.swapchain);
        self.shared.wanted.fetch_sub(1, Ordering::AcqRel);
        held
    }

    /// Takes `swapchain` as the chain's swap chain, whose present ids start again from 1 and
    /// whose presents are numbered after the `before` presents of earlier swap chains.
    ///
    /// The swap chain must be held through [`hold`](Self::hold).
    pub(super) fn replaced(&self, swapchain: vk::SwapchainKHR, before: u64) {
        let mut state = lock(&self.shared.state);
        state.swapchain = swapchain;
        state.generation += 1;
        state.before = before;
        state.submitted = 0;
        state.waited = 0;
        state.queued.clear();
    }

    /// Notes that the present with `id` was submitted.
    pub(super) fn submitted(&self, id: u64) {
        {
            let mut state = lock(&self.shared.state);
            let alone = state.waited >= state.submitted;
            state.submitted = id;
            state.queued.push_back(Queued {
                id,
                at_ns: iceblink::now_ns(),
                alone,
            });
        }
        self.shared.changed.notify_all();
    }

    /// Takes the refreshes frames are planned on: one at `refresh_ns` and the others every
    /// `interval_ns` from it, on the frame clock.
    pub(super) fn follow(&self, refresh_ns: u64, interval_ns: u64) {
        lock(&self.shared.state)
            .refreshes
            .follow(refresh_ns, interval_ns);
    }

    /// Reports whether as many presents wait for the display as it holds at once, so a present
    /// submitted now would queue behind them.
    ///
    /// A present given up on no longer waits.
    pub(super) fn full(&self) -> bool {
        let state = lock(&self.shared.state);
        let waiting = state.submitted.saturating_sub(state.waited);
        waiting >= state.depth.get(state.refreshes.interval())
    }

    /// Returns how many presents the display holds at once.
    pub(super) fn depth(&self) -> u32 {
        let state = lock(&self.shared.state);
        let depth = state.depth.get(state.refreshes.interval());
        u32::try_from(depth).unwrap_or(u32::MAX)
    }

    /// Moves the numbers of presents and when they were shown on the display into `out`, oldest
    /// first.
    pub(super) fn drain(&self, out: &mut Vec<(u64, u64)>) {
        out.append(&mut lock(&self.shared.state).presented);
    }
}

impl Drop for PresentTiming {
    fn drop(&mut self) {
        lock(&self.shared.state).stop = true;
        self.shared.changed.notify_all();
        if let Some(waiter) = self.waiter.take() {
            let _ = waiter.join();
        }
    }
}

/// Locks `mutex`, whether or not it is poisoned.
// The state holds no invariant a panic between two writes could break.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
