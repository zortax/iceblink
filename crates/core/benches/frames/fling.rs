//! A kinetic fling over a long list, and the cost of each frame it moves.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iceblink::Compositor;
use iceblink::{
    BufferAge, Built, FrameClock, FrameOutcome, FrameRequest, FrameStats, GesturePhase,
    ScrollBehaviour, ScrollGesture, ScrollNode, ScrollPhysics, ScrollSource, SnapKind, Sources,
};

use crate::list::{CAPS, List, ROW, Rows, SIDE};

/// The refresh interval of the frames, in nanoseconds.
const FRAME: u64 = 16_666_667;

/// The release velocity of each fling, in device pixels per second.
const VELOCITY: f32 = 4000.0;

/// A list the compositor flings, restarted from the top whenever a fling comes to rest.
pub(crate) struct Fling {
    list: List,
    /// The present time of the next frame.
    now_ns: u64,
    /// The present time the current fling comes to rest by.
    rest_ns: u64,
}

impl Fling {
    pub(crate) fn new(rows: Rows) -> Self {
        let mut list = List::new(rows);
        let mut tx = list.compositor.transaction(list.client);
        let [width, height] = SIDE.map(|side| side as f32);
        tx.set_scroll_node(
            list.scroll,
            ScrollNode {
                space: list.space,
                port: [0.0, 0.0, width, height],
                content: [width, rows.count as f32 * ROW],
                offset: [0.0, 0.0],
                behaviour: Some(ScrollBehaviour {
                    physics: ScrollPhysics::Kinetic,
                    snap: Arc::from([]),
                    snap_kind: SnapKind::Mandatory,
                    bounce: Sources::NONE,
                    user_scrollable: [false, true],
                    chain: Sources::NONE,
                }),
            },
        );
        list.compositor.commit(tx).expect("the behaviour commits");
        list.draw();

        let mut fling = Self {
            list,
            now_ns: FRAME,
            rest_ns: 0,
        };
        fling.restart();
        fling
    }

    /// Scrolls back to the top and starts a new fling.
    fn restart(&mut self) {
        let list = &mut self.list;
        let mut tx = list.compositor.transaction(list.client);
        tx.scroll_to(list.scroll, [0.0, 0.0]);
        list.compositor.commit(tx).expect("the scroll commits");
        self.tick();

        for (delta, phase) in [
            (0.0, GesturePhase::Begin),
            (20.0, GesturePhase::Update),
            (
                0.0,
                GesturePhase::End {
                    velocity: [0.0, VELOCITY],
                },
            ),
        ] {
            self.list.compositor.scroll_input(
                self.list.output,
                ScrollGesture {
                    position: [400.0, 300.0],
                    delta: [0.0, delta],
                    phase,
                    source: ScrollSource::Touchpad,
                    time_ns: self.now_ns,
                },
            );
        }

        // The fling slows below a pixel per refresh, `u`, after `ln(u / v) / ln(0.002)` seconds.
        let rest = 1e9 / FRAME as f64;
        let stop_s = (rest / f64::from(VELOCITY)).ln() / 0.002_f64.ln();
        self.rest_ns = self.now_ns + (stop_s * 1e9) as u64;
    }

    /// Builds the next frame and reports it presented.
    fn tick(&mut self) -> Built {
        let request = FrameRequest {
            caps: &CAPS,
            age: BufferAge::Kept,
            clock: FrameClock {
                now_ns: self.now_ns - FRAME,
                present_ns: self.now_ns,
                interval_ns: FRAME,
            },
            subpixel: None,
        };
        self.now_ns += FRAME;

        let list = &mut self.list;
        let built = list
            .compositor
            .build_frame(list.output, &request, &mut list.frame);
        if built == Built::Frame {
            let id = list.frame.id;
            list.compositor.frame_done(
                list.output,
                id,
                &FrameOutcome::Presented(FrameStats::default()),
            );
        }
        list.compositor.drain_events(&mut Vec::new());
        built
    }

    /// Returns the time `iterations` frames of flings take, with restarts outside it.
    pub(crate) fn time_ticks(&mut self, iterations: u64) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            if self.now_ns >= self.rest_ns {
                self.restart();
            }
            let start = Instant::now();
            std::hint::black_box(self.tick());
            total += start.elapsed();
        }
        total
    }

    /// Prints the counters of the second frame of a fling.
    pub(crate) fn report(&mut self, name: &str) {
        self.tick();
        self.tick();
        let counters = self.list.compositor.counters().frame;
        println!(
            "{name}: frame: chunks_drawn {}, items_drawn {}, order_nodes_visited {}, damage_px {}, \
             shifted_px {}, shift_refusals {}",
            counters.chunks_drawn,
            counters.items_drawn,
            counters.order_nodes_visited,
            counters.damage_px,
            counters.shifted_px,
            counters.shift_refusals,
        );
    }
}
