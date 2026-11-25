//! Running rasteriser batches, on scoped worker threads where the target has them.

use iceblink::GlyphRaster;

use super::batch::Batch;
use crate::content::glyph::answers::RasterAnswer;
use crate::content::glyph::raster::answer_of;

/// The answers of every batch, in batch order, and how many batches ran on worker threads.
pub(super) struct Ran {
    pub(super) answers: Vec<Vec<RasterAnswer>>,
    pub(super) on_workers: usize,
}

/// Returns the number of threads that can run at once, at least one.
pub(super) fn available() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism().map_or(1, usize::from)
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

/// Rasterises every batch, on worker threads when `parallel` holds and more than one thread can
/// run.
pub(super) fn rasterize(raster: &dyn GlyphRaster, batches: &[Batch], parallel: bool) -> Ran {
    let workers = available().min(batches.len());
    if !parallel || workers <= 1 {
        return Ran {
            answers: batches.iter().map(|batch| run(raster, batch)).collect(),
            on_workers: 0,
        };
    }
    on_threads(raster, batches, workers)
}

/// Rasterises the batches on `workers` scoped threads, each taking the next batch until none is
/// left.
#[cfg(not(target_arch = "wasm32"))]
fn on_threads(raster: &dyn GlyphRaster, batches: &[Batch], workers: usize) -> Ran {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let next = AtomicUsize::new(0);
    let mut answers: Vec<Option<Vec<RasterAnswer>>> = (0..batches.len()).map(|_| None).collect();

    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(batch) = batches.get(index) else {
                            break;
                        };
                        done.push((index, run(raster, batch)));
                    }
                    done
                })
            })
            .collect();
        for handle in handles {
            // A panic in the rasteriser is a panic of the caller.
            let done = handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            for (index, batch) in done {
                answers[index] = Some(batch);
            }
        }
    });

    Ran {
        answers: answers.into_iter().map(Option::unwrap_or_default).collect(),
        on_workers: batches.len(),
    }
}

#[cfg(target_arch = "wasm32")]
fn on_threads(raster: &dyn GlyphRaster, batches: &[Batch], _workers: usize) -> Ran {
    Ran {
        answers: batches.iter().map(|batch| run(raster, batch)).collect(),
        on_workers: 0,
    }
}

/// Rasterises one batch with one `rasterize_many` call.
fn run(raster: &dyn GlyphRaster, batch: &Batch) -> Vec<RasterAnswer> {
    let keys = &batch.keys;
    let mut answers: Vec<Option<RasterAnswer>> = (0..keys.len()).map(|_| None).collect();
    raster.rasterize_many(
        &batch.font,
        &batch.coords,
        keys,
        &mut |index, placement, bitmap| {
            if let Some(entry) = answers.get_mut(index) {
                *entry = Some(answer_of(&keys[index], placement, bitmap));
            }
        },
    );

    answers
        .into_iter()
        .map(|answer| {
            answer.unwrap_or(RasterAnswer {
                placement: Default::default(),
                size: [0, 0],
                texels: None,
            })
        })
        .collect()
}
