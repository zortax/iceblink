//! The cost of the threaded driver against the in-process driver, on one stepped workload.
//!
//! Every mode runs the same frames: a touchpad drag over a list of 2000 rows on an 800 by 600
//! output, whose client recycles a chunk of 25 rows every fourth frame, drawn by a null renderer.
//! The steps carry a virtual clock, so the compositor builds the same frames in every mode, and
//! each round checks that every mode built the same frames, applied the same commits and showed
//! the same final offset before it reports anything.
//!
//! | Mode | Driver | Pacing |
//! | --- | --- | --- |
//! | `local` | in process | back to back |
//! | `thread` | driver thread | back to back, the client waits for each step |
//! | `watched` | driver thread | as `thread`, with a scroll watch and a frame watch that fire every frame |
//! | `local-rt` | in process | one frame per interval of wall time |
//! | `thread-rt` | driver thread | one frame per interval of wall time, the client never waits |
//!
//! For each mode it prints the medians over the rounds, per frame: the client thread's wall time
//! (back to back only), the on-CPU time of the client thread, of the driver thread and of both,
//! how often the driver thread ran, how often the client was woken, and the watch and frame
//! events it took. Outside `watched` the client watches nothing, so its wake count is the one a
//! frame causes without a subscription firing, and `watched` against `thread` is the cost of
//! delivering events that wake the client.
//!
//! `DRIVER_BENCH_FRAMES` sets the frames per run (240), `DRIVER_BENCH_ROUNDS` the rounds (5) and
//! `DRIVER_BENCH_HZ` the rate of the real-time modes (120).

mod cpu;
mod workload;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use iceblink::CompositorClient;
use iceblink::FrameClock;
use iceblink_core::CompositorImpl;
use iceblink_driver::DriverOptions;
use iceblink_driver::InProcess;
use iceblink_driver::thread::ThreadedCompositor;

use crate::cpu::{Sched, named, sched, this_thread};
use crate::workload::{Seen, Workload};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Local,
    Thread,
    Watched,
    LocalRealTime,
    ThreadRealTime,
}

const MODES: [Mode; 5] = [
    Mode::Local,
    Mode::Thread,
    Mode::Watched,
    Mode::LocalRealTime,
    Mode::ThreadRealTime,
];

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Thread => "thread",
            Self::Watched => "watched",
            Self::LocalRealTime => "local-rt",
            Self::ThreadRealTime => "thread-rt",
        }
    }

    fn threaded(self) -> bool {
        matches!(self, Self::Thread | Self::Watched | Self::ThreadRealTime)
    }

    fn real_time(self) -> bool {
        matches!(self, Self::LocalRealTime | Self::ThreadRealTime)
    }
}

/// What one run of a mode measured, per frame.
#[derive(Clone, Copy, Debug, Default)]
struct Run {
    wall_ns: f64,
    client_ns: f64,
    driver_ns: f64,
    driver_runs: f64,
    wakes: f64,
    notices: f64,
}

fn main() {
    let frames = setting("DRIVER_BENCH_FRAMES", 240);
    let rounds = setting("DRIVER_BENCH_ROUNDS", 5);
    let hz = setting("DRIVER_BENCH_HZ", 120);
    let interval_ns = 1_000_000_000 / hz;

    let mut runs: Vec<Vec<Run>> = vec![Vec::new(); MODES.len()];
    for round in 0..rounds {
        let mut seen = Vec::new();
        for (index, mode) in MODES.into_iter().enumerate() {
            let (run, saw) = measure(mode, frames, interval_ns);
            runs[index].push(run);
            seen.push(saw);
        }
        assert!(
            seen.windows(2).all(|pair| pair[0] == pair[1]),
            "round {round}: the modes ran different workloads: {seen:?}"
        );
        if round == 0 {
            let saw = seen[0];
            println!(
                "workload: {} frames built, {} commits, final offset {}",
                saw.frames, saw.committed, saw.offset
            );
        }
    }

    println!(
        "{:<10} {:>10} {:>10} {:>10} {:>10} {:>12} {:>8} {:>8}",
        "mode", "wall us", "client us", "driver us", "total us", "driver runs", "wakes", "events"
    );
    for (index, mode) in MODES.into_iter().enumerate() {
        let median = |field: fn(&Run) -> f64| median(runs[index].iter().map(field).collect());
        let wall = if mode.real_time() {
            "-".to_owned()
        } else {
            format!("{:.1}", median(|run| run.wall_ns) / 1e3)
        };
        let client = median(|run| run.client_ns);
        let driver = median(|run| run.driver_ns);
        println!(
            "{:<10} {:>10} {:>10.1} {:>10.1} {:>10.1} {:>12.2} {:>8.2} {:>8.2}",
            mode.name(),
            wall,
            client / 1e3,
            driver / 1e3,
            median(|run| run.client_ns + run.driver_ns) / 1e3,
            median(|run| run.driver_runs),
            median(|run| run.wakes),
            median(|run| run.notices),
        );
    }
}

/// Runs `frames` frames of the workload in `mode`, and returns the cost per frame and what the
/// client saw.
fn measure(mode: Mode, frames: u64, interval_ns: u64) -> (Run, Seen) {
    let compositor = Box::new(CompositorImpl::new());
    let mut driver: Box<dyn CompositorClient> = if mode.threaded() {
        let (driver, _) = ThreadedCompositor::spawn_with(compositor, DriverOptions::stepped());
        Box::new(driver)
    } else {
        Box::new(InProcess::new(compositor, DriverOptions::stepped()))
    };
    let mut workload = Workload::open(&mut *driver);
    let wakes = Arc::new(AtomicU64::new(0));
    let counted = Arc::clone(&wakes);
    driver.set_waker(
        workload.client(),
        Some(Arc::new(move || {
            counted.fetch_add(1, Ordering::Relaxed);
        })),
    );
    if mode == Mode::Watched {
        workload.watch(&mut *driver);
    }
    driver.sync();
    workload.take(&mut *driver);
    let committed = workload.seen.committed;

    let client = this_thread();
    let driven = mode.threaded().then(|| named("compositor")).flatten();
    let read = |tid: Option<u32>| tid.map_or_else(Sched::default, sched);
    let (client_start, driver_start) = (read(Some(client)), read(driven));
    let started = Instant::now();
    for index in 0..frames {
        if mode.real_time() {
            let due = started + Duration::from_nanos(index * interval_ns);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                thread::sleep(wait);
            }
        }
        let clock = FrameClock {
            now_ns: index * interval_ns,
            present_ns: (index + 1) * interval_ns,
            interval_ns,
        };
        workload.frame(&mut *driver, index, clock);
        if matches!(mode, Mode::Thread | Mode::Watched) {
            driver.sync();
        }
        workload.take(&mut *driver);
    }
    driver.sync();
    workload.take(&mut *driver);
    let wall = started.elapsed();
    let (client_used, driver_used) = (
        read(Some(client)).since(client_start),
        read(driven).since(driver_start),
    );
    drop(driver);

    let per_frame = |value: u64| value as f64 / frames as f64;
    let run = Run {
        wall_ns: wall.as_nanos() as f64 / frames as f64,
        client_ns: per_frame(client_used.on_cpu_ns),
        driver_ns: per_frame(driver_used.on_cpu_ns),
        driver_runs: per_frame(driver_used.runs),
        wakes: per_frame(wakes.load(Ordering::Relaxed)),
        notices: per_frame(workload.notices),
    };
    let mut seen = workload.seen;
    seen.committed -= committed;
    (run, seen)
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
