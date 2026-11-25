//! Drawing a frame's batches into the composed target, once per draw rectangle, in bands drawn
//! on worker threads.

pub(crate) mod backdrop;
pub(crate) mod filter;
mod group;
pub(crate) mod instances;
pub(crate) mod pool;
pub(crate) mod rects;
mod replay;
pub(crate) mod resident;

use std::sync::{Arc, Mutex};
use std::thread;

use iceblink::geom::{Device, Rect};
use iceblink::{Frame, OpaqueCover};

use crate::draw::Scene;
use crate::draw::marks::Bins;
use crate::external::Externals;
use crate::frame::instances::Instances;
use crate::frame::pool::Pool;
use crate::target::Surface;
use crate::vector::Raster;

/// The frame, tables and rasters that a frame's batches are replayed with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Context<'a> {
    pub(crate) frame: &'a Frame,
    pub(crate) scene: Scene<'a>,
    pub(crate) instances: &'a Instances,
    pub(crate) bins: &'a Bins,
    /// The raster of every vector pass, by pass index.
    pub(crate) rasters: &'a [Arc<Raster>],
    pub(crate) externals: &'a Externals,
    /// The pixels of the composed target in use.
    pub(crate) used: Rect<i32, Device>,
    /// How far each group's region extends so its backdrops read what was drawn, by group.
    pub(crate) nested: &'a [i32],
    /// The opaque covers a rectangle may start at.
    pub(crate) covers: &'a [OpaqueCover],
}

/// The state one worker thread replays bands with.
#[derive(Debug, Default)]
pub(crate) struct Worker {
    pub(crate) pool: Pool,
    pub(crate) counts: Counts,
    row: Vec<[f32; 4]>,
    coverage: Vec<[f32; 3]>,
}

/// The batches a replay drew and culled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Batches drawn, summed over bands.
    pub(crate) draws: u32,
    /// Instanced batches left out of a band their ink misses.
    pub(crate) culled: u32,
}

/// Replays the frame's batches over every rectangle of `rects` into `composed`.
///
/// A rectangle large enough is split into bands, which the workers replay in parallel. Each band
/// draws what a whole replay draws in its rows.
pub(crate) fn draw(
    context: &Context<'_>,
    composed: &mut Surface<'_>,
    rects: &[Rect<i32, Device>],
    workers: &mut [Worker],
) {
    for &rect in rects {
        let bands = rects::bands(context.frame, rect, workers.len());
        if bands.len() <= 1 {
            replay::replay(context, &mut workers[0], composed, rect);
            continue;
        }
        replay_bands(context, composed, rect, &bands, workers);
    }
}

// Replays `bands`, the bands of `rect`, on the workers in parallel.
fn replay_bands(
    context: &Context<'_>,
    composed: &mut Surface<'_>,
    rect: Rect<i32, Device>,
    bands: &[Rect<i32, Device>],
    workers: &mut [Worker],
) {
    let mut slices = Vec::with_capacity(bands.len());
    let (_, mut rest) = composed.reborrow().split_rows(rect.top());
    for band in bands {
        let (slice, below) = rest.split_rows(band.bottom());
        slices.push((*band, slice));
        rest = below;
    }

    // Workers take the next band as they finish one, so a slow core holds back no other.
    let queue = Mutex::new(slices.into_iter());
    let next = || queue.lock().map_or(None, |mut queue| queue.next());
    thread::scope(|scope| {
        for worker in workers.iter_mut() {
            let next = &next;
            scope.spawn(move || {
                while let Some((band, mut surface)) = next() {
                    replay::replay(context, worker, &mut surface, band);
                }
            });
        }
    });
}
