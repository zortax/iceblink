//! Scrolling a list one row at a time, and the cost of each scroll's commit and frame.

use std::time::{Duration, Instant};

use iceblink::Built;
use iceblink::Compositor;

use crate::list::{List, ROW, Rows};

/// A list scrolled by one row per step, back and forth between its ends.
pub(crate) struct Scroller {
    list: List,
    offset: f32,
    step: f32,
}

impl Scroller {
    pub(crate) fn new(rows: Rows) -> Self {
        Self {
            list: List::new(rows),
            offset: 0.0,
            step: ROW,
        }
    }

    /// Commits a scroll by one row.
    pub(crate) fn commit(&mut self) {
        if !(0.0..=self.list.end()).contains(&(self.offset + self.step)) {
            self.step = -self.step;
        }
        self.offset += self.step;

        let list = &mut self.list;
        let mut tx = list.compositor.transaction(list.client);
        tx.scroll_to(list.scroll, [0.0, self.offset]);
        list.compositor.commit(tx).expect("the scroll commits");
    }

    /// Builds the frame of the last scroll.
    pub(crate) fn draw(&mut self) -> Built {
        self.list.draw()
    }

    /// Returns the time `iterations` scroll commits take, with their frames built outside it.
    pub(crate) fn time_commits(&mut self, iterations: u64) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            let start = Instant::now();
            self.commit();
            total += start.elapsed();
            self.draw();
        }
        total
    }

    /// Returns the time the frames of `iterations` scrolls take, with their commits outside it.
    pub(crate) fn time_frames(&mut self, iterations: u64) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            self.commit();
            let start = Instant::now();
            std::hint::black_box(self.draw());
            total += start.elapsed();
        }
        total
    }

    /// Prints the work counters of one scroll's commit and the counters of its frame.
    pub(crate) fn report(&mut self, name: &str) {
        let before = self.work();
        self.commit();
        let commit = self.work();
        assert_eq!(self.draw(), Built::Frame);
        let frame = self.work();
        let counters = self.list.compositor.counters().frame;

        let delta = |from: &[(&'static str, u64)], to: &[(&'static str, u64)]| {
            to.iter()
                .zip(from)
                .map(|((name, after), (_, before))| format!("{name} {}", after - before))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!("{name}: commit: {}", delta(&before, &commit));
        println!("{name}: frame: {}", delta(&commit, &frame));
        println!(
            "{name}: frame: chunks_drawn {}, items_drawn {}, items_culled {}, \
             order_nodes_visited {}, damage_px {}, shifted_px {}, shift_refusals {}",
            counters.chunks_drawn,
            counters.items_drawn,
            counters.items_culled,
            counters.order_nodes_visited,
            counters.damage_px,
            counters.shifted_px,
            counters.shift_refusals,
        );
    }

    /// Returns the compositor's work counters.
    fn work(&self) -> Vec<(&'static str, u64)> {
        let mut work = Vec::new();
        self.list.compositor.extra_stats(&mut |name, value| {
            if name.starts_with("work.") {
                work.push((name, value));
            }
        });
        work
    }
}
