//! On-CPU time and runs of threads, read from the scheduler's statistics.

use std::fs;

/// What the scheduler counted for one thread: nanoseconds on a CPU and times it ran.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Sched {
    pub(crate) on_cpu_ns: u64,
    pub(crate) runs: u64,
}

impl Sched {
    pub(crate) fn since(self, earlier: Self) -> Self {
        Self {
            on_cpu_ns: self.on_cpu_ns - earlier.on_cpu_ns,
            runs: self.runs - earlier.runs,
        }
    }
}

/// Reads the statistics of thread `tid` of this process, or zeros where the kernel offers none.
pub(crate) fn sched(tid: u32) -> Sched {
    let Ok(line) = fs::read_to_string(format!("/proc/self/task/{tid}/schedstat")) else {
        return Sched::default();
    };
    let mut fields = line
        .split_whitespace()
        .map(|field| field.parse().unwrap_or(0));
    let on_cpu_ns = fields.next().unwrap_or(0);
    let _waiting = fields.next();
    let runs = fields.next().unwrap_or(0);
    Sched { on_cpu_ns, runs }
}

/// Returns the id of the calling thread.
pub(crate) fn this_thread() -> u32 {
    fs::read_link("/proc/thread-self")
        .ok()
        .and_then(|path| path.file_name()?.to_str()?.parse().ok())
        .unwrap_or(0)
}

/// Returns the id of the thread named `name`, if one is running.
pub(crate) fn named(name: &str) -> Option<u32> {
    fs::read_dir("/proc/self/task")
        .ok()?
        .flatten()
        .find(|task| {
            fs::read_to_string(task.path().join("comm")).is_ok_and(|comm| comm.trim() == name)
        })
        .and_then(|task| task.file_name().to_str()?.parse().ok())
}
