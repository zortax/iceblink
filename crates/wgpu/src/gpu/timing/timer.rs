//! The timestamp queries of a renderer's frames, and their readback.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::gpu::device::Gpu;
use crate::gpu::timing::{GpuTiming, TimedWork, requested};

/// How many timestamp queries one frame has, two per timed pass.
const QUERIES: u32 = 512;

/// How many frames are timed at once.
const SLOTS: usize = 3;

/// How many finished timings are kept until they are taken.
const KEPT: usize = 1024;

/// The timestamp queries of one timed pass.
pub(crate) struct PassQueries {
    set: wgpu::QuerySet,
    first: u32,
}

impl PassQueries {
    /// Returns the timestamp writes of a render pass.
    pub(crate) fn render(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(self.first),
            end_of_pass_write_index: Some(self.first + 1),
        }
    }
}

/// Where a slot is in its cycle.
enum State {
    Free,
    Recording,
    /// The readback is mapping, with its flag: 0 while pending, 1 once mapped and 2 on failure.
    Mapping(Arc<AtomicU8>),
}

/// The queries and readback buffers of one timed frame.
struct Slot {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    state: State,
    used: u32,
    work: Vec<TimedWork>,
    frame: u64,
    untimed: u32,
}

impl Slot {
    fn new(gpu: &Gpu) -> Self {
        let device = gpu.device();
        let bytes = u64::from(QUERIES) * u64::from(wgpu::QUERY_SIZE);
        Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("iceblink.timing.queries"),
                ty: wgpu::QueryType::Timestamp,
                count: QUERIES,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("iceblink.timing.resolve"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("iceblink.timing.readback"),
                size: bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            state: State::Free,
            used: 0,
            work: Vec::new(),
            frame: 0,
            untimed: 0,
        }
    }

    /// Returns the bytes of the timestamps written so far.
    fn bytes(&self) -> u64 {
        u64::from(self.used) * u64::from(wgpu::QUERY_SIZE)
    }
}

/// Timestamp queries around each frame's passes, read back once the device finished them.
pub(crate) struct GpuTimer {
    slots: Vec<Slot>,
    recording: Option<usize>,
    period_ns: f64,
    frames: u64,
    done: VecDeque<GpuTiming>,
    latest: Option<GpuTiming>,
}

impl GpuTimer {
    /// Returns a timer on `gpu`, or `None` when timing was not requested or the device has no
    /// timestamp queries.
    pub(crate) fn new(gpu: &Gpu) -> Option<Self> {
        if !requested()
            || !gpu
                .device()
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        Some(Self {
            slots: (0..SLOTS).map(|_| Slot::new(gpu)).collect(),
            recording: None,
            period_ns: f64::from(gpu.queue().get_timestamp_period()),
            frames: 0,
            done: VecDeque::new(),
            latest: None,
        })
    }

    pub(crate) fn frames(&self) -> u64 {
        self.frames
    }

    pub(crate) fn latest(&self) -> Option<GpuTiming> {
        self.latest
    }

    /// Collects the finished timings and starts timing a frame.
    ///
    /// The frame is untimed when no slot is free.
    pub(crate) fn begin(&mut self, gpu: &Gpu) {
        self.collect(gpu);
        let frame = self.frames;
        self.frames += 1;
        self.recording = self
            .slots
            .iter()
            .position(|slot| matches!(slot.state, State::Free));
        if let Some(index) = self.recording {
            let slot = &mut self.slots[index];
            slot.state = State::Recording;
            slot.used = 0;
            slot.work.clear();
            slot.frame = frame;
            slot.untimed = 0;
        }
    }

    /// Returns the queries of the next pass of `work`, or `None` when the frame is untimed or out
    /// of queries.
    pub(crate) fn pass(&mut self, work: TimedWork) -> Option<PassQueries> {
        let slot = &mut self.slots[self.recording?];
        if slot.used + 2 > QUERIES {
            slot.untimed += 1;
            return None;
        }
        let first = slot.used;
        slot.used += 2;
        slot.work.push(work);
        Some(PassQueries {
            set: slot.queries.clone(),
            first,
        })
    }

    /// Writes the timestamp that opens the frame's vector rasterisation in a submission of its
    /// own, and returns its queries.
    pub(crate) fn open_vectors(&mut self, gpu: &Gpu) -> Option<PassQueries> {
        let queries = self.pass(TimedWork::Vectors)?;
        marker(gpu, &queries.set, None, Some(queries.first));
        Some(queries)
    }

    /// Writes the timestamp that closes the vector rasterisation of `queries`.
    pub(crate) fn close_vectors(gpu: &Gpu, queries: &PassQueries) {
        marker(gpu, &queries.set, Some(queries.first + 1), None);
    }

    /// Records the copy of the frame's timestamps into its readback buffer.
    pub(crate) fn finish(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(index) = self.recording else {
            return;
        };
        let slot = &mut self.slots[index];
        if slot.used == 0 {
            slot.state = State::Free;
            self.recording = None;
            return;
        }
        encoder.resolve_query_set(&slot.queries, 0..slot.used, &slot.resolve, 0);
        encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, slot.bytes());
    }

    /// Starts mapping the frame's readback buffer, once the frame's encoder is submitted.
    pub(crate) fn submitted(&mut self) {
        let Some(index) = self.recording.take() else {
            return;
        };
        let slot = &mut self.slots[index];
        let flag = Arc::new(AtomicU8::new(0));
        let mapped = Arc::clone(&flag);
        slot.readback
            .slice(0..slot.bytes())
            .map_async(wgpu::MapMode::Read, move |result| {
                mapped.store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        slot.state = State::Mapping(flag);
    }

    /// Reads the timings whose readback finished, without waiting.
    pub(crate) fn collect(&mut self, gpu: &Gpu) {
        let pending = |slot: &Slot| match &slot.state {
            State::Mapping(flag) => flag.load(Ordering::Acquire) == 0,
            State::Free | State::Recording => false,
        };
        if self.slots.iter().any(pending) {
            let _ = gpu.device().poll(wgpu::PollType::Poll);
        }
        let mut completed = Vec::new();
        for slot in &mut self.slots {
            let State::Mapping(flag) = &slot.state else {
                continue;
            };
            match flag.load(Ordering::Acquire) {
                0 => continue,
                1 => {
                    // A buffer that went with a lost device cannot be read, and unmapping it is a
                    // validation error. Its timings are dropped, as for a failed map.
                    if let Some(timing) = read(slot, self.period_ns) {
                        completed.push(timing);
                        slot.readback.unmap();
                    }
                }
                _ => {}
            }
            slot.state = State::Free;
        }
        completed.sort_by_key(|timing| timing.frame);
        for timing in completed {
            if self.done.len() == KEPT {
                self.done.pop_front();
            }
            self.done.push_back(timing);
            self.latest = Some(timing);
        }
    }

    /// Collects the finished timings, and returns the ones not taken before, oldest first.
    ///
    /// At most the newest 1024 are kept between calls.
    pub(crate) fn take(&mut self, gpu: &Gpu) -> Vec<GpuTiming> {
        self.collect(gpu);
        self.done.drain(..).collect()
    }
}

/// Returns the timing of the mapped `slot`, or `None` when its readback cannot be read.
fn read(slot: &Slot, period_ns: f64) -> Option<GpuTiming> {
    let mut timing = GpuTiming {
        frame: slot.frame,
        untimed: slot.untimed,
        ..GpuTiming::default()
    };
    let view = slot
        .readback
        .slice(0..slot.bytes())
        .get_mapped_range()
        .ok()?;
    let mut stamps = view
        .as_chunks::<8>()
        .0
        .iter()
        .map(|&word| u64::from_le_bytes(word));
    for &work in &slot.work {
        let (Some(begin), Some(end)) = (stamps.next(), stamps.next()) else {
            break;
        };
        timing.add(work, end.saturating_sub(begin), period_ns);
    }
    Some(timing)
}

/// Submits an empty pass that writes the timestamps `begin` and `end` of `set`.
fn marker(gpu: &Gpu, set: &wgpu::QuerySet, begin: Option<u32>, end: Option<u32>) {
    let mut encoder = gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("iceblink.timing.marker"),
        });
    drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("iceblink.timing.marker"),
        timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
            query_set: set,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: end,
        }),
    }));
    gpu.queue().submit([encoder.finish()]);
}
