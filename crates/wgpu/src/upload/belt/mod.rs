//! The staging belt and its chunks.

mod chunk;
mod write;

use std::sync::mpsc;

use self::chunk::Chunk;

pub use self::write::TexelWrite;

/// A mapping the device refused, which leaves the frame's uploads incomplete.
#[derive(Debug)]
pub struct Refused(pub wgpu::MapRangeError);

/// A staging belt that grows on demand and drops chunks that stay unused.
///
/// Writes are suballocated from mapped chunks and copied with the frame's encoder. Chunks come
/// back for reuse once the device has finished the copies that read them, and a free chunk
/// unused for 120 frames is dropped, except the smallest one.
///
/// A write of a mebibyte or more is split into pieces of at most a mebibyte, in whole rows for a
/// texture. The pieces go into one-shot chunks, which are dropped once their copies are
/// submitted.
// `Queue::write_texture` allocates and maps a staging buffer per call, and on the GL backend that
// waits for the card while it draws.
pub struct UploadBelt {
    /// Chunks written this frame.
    active: Vec<Chunk>,
    /// Chunks unmapped for submission.
    closed: Vec<Chunk>,
    /// Chunks mapped again and ready for writes.
    free: Vec<Chunk>,
    /// Mapping callback results: the chunk's size, and the chunk if its mapping succeeded.
    sender: mpsc::Sender<(u64, Option<Chunk>)>,
    receiver: mpsc::Receiver<(u64, Option<Chunk>)>,
    frame: u64,
    /// Bytes of chunks whose mapping is in flight.
    mapping_bytes: u64,
    /// Chunks allocated since the belt was made.
    allocations: u64,
    /// Bytes written since the belt was made.
    uploaded: u64,
}

impl std::fmt::Debug for UploadBelt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UploadBelt")
            .field("active", &self.active.len())
            .field("closed", &self.closed.len())
            .field("free", &self.free.len())
            .field("bytes", &self.bytes())
            .finish_non_exhaustive()
    }
}

impl Default for UploadBelt {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            active: Vec::new(),
            closed: Vec::new(),
            free: Vec::new(),
            sender,
            receiver,
            frame: 0,
            mapping_bytes: 0,
            allocations: 0,
            uploaded: 0,
        }
    }
}

impl UploadBelt {
    /// The smallest chunk.
    // Large enough to combine an ordinary frame's writes into one mapping.
    const MINIMUM: u64 = 256 * 1024;

    /// The largest piece of a write, and the size from which a write gets one-shot chunks.
    const MAXIMUM: u64 = 1 << 20;

    /// How many frames a free chunk stays unused before it is dropped.
    const RETAIN_FRAMES: u64 = 120;

    /// Starts a frame, and takes back the chunks whose mapping has completed.
    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.receive();

        // One chunk stays warm. The others stay only while they were used recently.
        let keep = self
            .free
            .iter()
            .enumerate()
            .min_by_key(|(_, chunk)| chunk.size)
            .map(|(index, _)| index);
        let mut index = 0_usize;
        self.free.retain(|chunk| {
            let retained = Some(index) == keep
                || self.frame.saturating_sub(chunk.last_used) < Self::RETAIN_FRAMES;
            index += 1;
            retained
        });
    }

    /// Unmaps every chunk written this frame, so the encoder that copies from them can be
    /// submitted.
    pub fn finish(&mut self) {
        for chunk in self.active.drain(..) {
            chunk.buffer.unmap();
            self.closed.push(chunk);
        }
    }

    /// Maps the submitted chunks again for reuse, without waiting for the device.
    ///
    /// One-shot chunks are dropped instead.
    pub fn recall(&mut self) {
        self.receive();
        for chunk in self.closed.drain(..) {
            if chunk.one_shot {
                continue;
            }
            self.mapping_bytes += chunk.size;
            let sender = self.sender.clone();
            let size = chunk.size;
            chunk
                .buffer
                .clone()
                .slice(..)
                .map_async(wgpu::MapMode::Write, move |result| {
                    // The size comes back even when the mapping failed, so the in-flight total
                    // falls again after a device loss.
                    let chunk = result.is_ok().then_some(chunk);
                    let _ = sender.send((size, chunk));
                });
        }
    }

    /// Returns the bytes of mapped and in-flight staging memory the belt holds.
    pub fn bytes(&self) -> u64 {
        self.active
            .iter()
            .chain(&self.closed)
            .chain(&self.free)
            .map(|chunk| chunk.size)
            .sum::<u64>()
            + self.mapping_bytes
    }

    /// Returns the bytes of the free chunks, which are mapped and wait for writes.
    pub fn warm_bytes(&self) -> u64 {
        self.free.iter().map(|chunk| chunk.size).sum()
    }

    /// Returns how many chunks were allocated since the belt was made.
    pub fn allocations(&self) -> u64 {
        self.allocations
    }

    /// Returns how many bytes were written since the belt was made.
    pub fn uploaded(&self) -> u64 {
        self.uploaded
    }

    /// Drops every free chunk, and returns the bytes freed.
    ///
    /// A chunk whose mapping callback has not run yet is kept.
    pub fn release_idle(&mut self) -> u64 {
        self.receive();
        let freed = self.free.iter().map(|chunk| chunk.size).sum();
        self.free.clear();
        self.free.shrink_to_fit();
        freed
    }

    /// Takes back the chunks whose mapping callback has run.
    fn receive(&mut self) {
        while let Ok((size, chunk)) = self.receiver.try_recv() {
            self.mapping_bytes = self.mapping_bytes.saturating_sub(size);
            if let Some(mut chunk) = chunk {
                chunk.offset = 0;
                self.free.push(chunk);
            }
        }
    }

    /// Reserves `size` bytes at `alignment`, and returns their offset in the chunk.
    ///
    /// The chunk is the last of the active chunks afterwards. A new chunk is one-shot when
    /// `one_shot` is set.
    fn reserve(&mut self, device: &wgpu::Device, size: u64, alignment: u64, one_shot: bool) -> u64 {
        self.receive();
        let fits = |chunk: &Chunk| chunk.allocation(size, alignment).is_some();
        let mut chunk = if let Some(index) = self.active.iter().position(fits) {
            self.active.swap_remove(index)
        } else if let Some(index) = self.free.iter().position(fits) {
            self.free.swap_remove(index)
        } else {
            self.allocate_chunk(device, size, one_shot)
        };
        let offset = chunk
            .allocation(size, alignment)
            .expect("a selected chunk has room for the write");
        chunk.offset = offset + size;
        chunk.last_used = self.frame;
        self.active.push(chunk);
        offset
    }

    /// Drops the chunk a refused write was reserved in, and returns the refusal.
    fn refuse(&mut self, error: wgpu::MapRangeError) -> Refused {
        // Unmapping a buffer wgpu no longer holds is itself a validation error, so the chunk is
        // not left for `finish`.
        self.active.pop();
        Refused(error)
    }

    /// Allocates a chunk for a write of `size` bytes that no held chunk has room for.
    fn allocate_chunk(&mut self, device: &wgpu::Device, size: u64, one_shot: bool) -> Chunk {
        self.allocations += 1;
        let chunk_size = if one_shot {
            size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT.max(wgpu::MAP_ALIGNMENT))
        } else {
            size.max(Self::MINIMUM).next_power_of_two()
        };
        Chunk {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("iceblink.upload.chunk"),
                size: chunk_size,
                usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: true,
            }),
            size: chunk_size,
            offset: 0,
            last_used: self.frame,
            one_shot,
        }
    }
}
