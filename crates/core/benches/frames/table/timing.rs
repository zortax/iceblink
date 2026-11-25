//! Frames, and the time the table's commits and frames take.

use std::time::{Duration, Instant};

use iceblink::Compositor;
use iceblink::{BufferAge, Built, FrameClock, FrameOutcome, FrameRequest, FrameStats};

use crate::list::CAPS;
use crate::table::Table;

impl Table {
    /// Builds a frame and reports it presented.
    // Out of line, so instruction profiles can count each call.
    #[inline(never)]
    pub(crate) fn draw(&mut self) -> Built {
        let request = FrameRequest {
            caps: &CAPS,
            age: BufferAge::Kept,
            clock: FrameClock::default(),
            subpixel: None,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built == Built::Frame {
            let outcome = FrameOutcome::Presented(FrameStats::default());
            self.compositor
                .frame_done(self.output, self.frame.id, &outcome);
        }
        built
    }

    /// Returns the time `iterations` changes that hide rows take when `hiding`, or that show
    /// them otherwise, with the other changes and every frame outside it.
    pub(crate) fn time_commits(&mut self, iterations: u64, hiding: bool) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            if self.hidden == hiding {
                self.commit();
                self.draw();
            }
            let start = Instant::now();
            self.commit();
            total += start.elapsed();
            self.draw();
        }
        total
    }

    /// Returns the time `iterations` caret blinks take, with their frames outside it.
    pub(crate) fn time_blinks(&mut self, iterations: u64) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            let start = Instant::now();
            self.blink();
            total += start.elapsed();
            self.draw();
        }
        total
    }

    /// Returns the time the frames after `iterations` changes take, with the changes outside
    /// it.
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

    /// Prints the commit counters of a change that hides rows and of one that shows them.
    pub(crate) fn report(&mut self, name: &str) {
        for _ in 0..2 {
            self.commit();
            let counters = self.compositor.counters().commit;
            let change = if self.hidden { "hide" } else { "show" };
            println!(
                "{name}: {change}: chunks_recorded {}, chunks_removed {}, keys_spliced {}, \
                 offsets_written {}",
                counters.chunks_recorded,
                counters.chunks_removed,
                counters.keys_spliced,
                counters.offsets_written,
            );
            self.draw();
        }
    }
}
