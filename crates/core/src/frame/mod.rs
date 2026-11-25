//! Building frames for outputs, and retiring them when the renderer reports their outcome.

mod age;
mod check;
mod closure;
mod counters;
mod damage;
mod done;
mod emit;
mod filters;
mod header;
pub(crate) mod keys;
pub(crate) mod lower;
mod notes;
mod plan;
mod returns;
mod shaders;
pub(crate) mod sources;
mod verify;
pub(crate) mod walk;

use iceblink::{Built, Frame, FrameRequest, OutputId};

use crate::compositor::CompositorImpl;
use crate::shared_content::prefetch::{RunOutput, prefetch_drawn};
use crate::tier::earliest;

use self::returns::route_wake;

pub use self::check::FrameCheck;

/// How many frames other outputs build before an idle output gets a frame of resource work.
const CATCH_UP_FRAMES: u64 = 64;

impl CompositorImpl {
    /// Builds the next frame of output `id` into `frame`.
    ///
    /// Returns `Built::Idle` when the output is unknown or has nothing to draw, and `Built::Frame`
    /// once the frame is built.
    pub(crate) fn build(
        &mut self,
        id: OutputId,
        request: &FrameRequest<'_>,
        frame: &mut Frame,
    ) -> Built {
        self.count_route_frame();
        let wake_ns = earliest(
            self.tick(id, &request.clock),
            route_wake(&self.content.cache, &request.clock),
        );
        if let Some(output) = self.outputs.get_mut(&id) {
            output.wake = wake_ns;
            output.interval_ns = request.clock.interval_ns;
        }

        let caps = *request.caps;
        let clock_start = *self.clock_start.get_or_insert(request.clock.now_ns);
        self.tables.combined.begin(self.serial);
        self.tables.refresh_combined();

        let Some(output) = self.outputs.get_mut(&id) else {
            return Built::Idle { wake_ns: None };
        };
        let root = output.root;
        let scene = root
            .and_then(|(client, _)| self.clients.get(&client))
            .map(|client| &client.scene);
        let root_slot = root.map(|(_, order)| order.slot());

        let (own, decision) = damage::own(output, scene, root_slot, request);
        let dead_shaders = output
            .renderer
            .shaders
            .iter()
            .any(|slot| !self.tables.shaders.contains_key(slot));
        // An idle output that other outputs left far behind gets a frame of resource work only,
        // so the logs and released slots it holds back are freed.
        let behind =
            !output.renderer.fresh && self.serial.saturating_sub(output.built) >= CATCH_UP_FRAMES;
        if own.is_empty() && decision.shifts.is_empty() && !dead_shaders && !behind {
            return Built::Idle { wake_ns };
        }

        if self.content.prepare(&mut output.renderer, &caps) {
            crate::shared_content::masks::refresh(
                &mut self.clients,
                &mut self.tables,
                &mut self.content,
            );
        }

        let Some(output) = self.outputs.get_mut(&id) else {
            return Built::Idle { wake_ns: None };
        };
        header::fill(frame, (id, output), request, clock_start, &decision, own);
        notes::begin(
            frame,
            &mut output.renderer,
            &self.revisions,
            caps.resident_chunks,
        );

        let scene = match root.and_then(|(client, _)| self.clients.get_mut(&client)) {
            Some(client) => &mut client.scene,
            None => &mut self.empty,
        };
        let text = walk::Text {
            subpixel: request
                .subpixel
                .filter(|_| caps.subpixel_text && output.config.opaque),
            scale: output.config.scale,
        };
        // Glyphs the walk would rasterise one at a time, after the atlas started over or the
        // output's text settings changed, are rasterised in parallel first.
        let settings = (text.subpixel, text.scale.to_bits(), self.content.epoch);
        if output.text != Some(settings) {
            let drawn = RunOutput {
                subpixel: text.subpixel,
                scale: text.scale,
            };
            prefetch_drawn(&mut self.content, scene, output.bounds(), drawn);
        }
        output.text = Some(settings);

        let mut parts = walk::Parts {
            scene,
            tables: &mut self.tables,
            revisions: &mut self.revisions,
            renderer: &mut output.renderer,
            content: &mut self.content,
            work: &mut self.work,
            pins: &mut output.pins,
        };
        let walked = walk::run(&mut parts, frame, caps, text, root_slot, &mut self.scratch);
        output.pins.settle(&mut self.content);
        if walked.glyph_runs_without_raster > 0 && !self.glyphs_missed {
            self.glyphs_missed = true;
            warn_unrasterised(walked.glyph_runs_without_raster);
        }

        let stats = plan::plan(
            &mut output.planner,
            &self.scratch,
            &mut self.tables,
            frame,
            self.serial,
            caps.kept_rasters,
        );
        if caps.kept_rasters {
            keys::assign(&mut frame.vector_plan, &frame.paths, &self.tables.paints);
            if self.verification {
                self.mismatches +=
                    self.pass_digests
                        .check(&frame.vector_plan, &frame.paths, &self.tables.paints);
            }
        }

        if self.verification {
            self.order_overlaps +=
                match output.planner.order_overlaps(crate::invariant::order_max()) {
                    Ok(overlaps) => overlaps.len() as u64,
                    Err(_) => 1,
                };
        }

        let bounds = output.bounds();
        self.tables.combined.sweep(&mut self.tables.clips);
        self.tables.deltas(&mut output.renderer, &mut frame.tables);
        shaders::work(
            &self.tables,
            &mut output.renderer,
            &self.scratch.shaders,
            frame,
        );
        self.content.finish(
            &mut output.renderer,
            self.budgets.atlas_bytes,
            &mut frame.resources,
        );

        let refusals = decision.refused.len() as u32;
        self.counters.frame = counters::of(frame, walked, &stats, refusals, bounds);
        for &refusal in &decision.refused {
            self.refusals.add(refusal);
        }
        self.work.moving_inks += decision.measured;

        output.sent(frame, own, self.serial);
        self.serial += 1;
        self.settle();
        if self.verification {
            self.verify_scenes(frame);
        }
        Built::Frame
    }

    /// Counts the derived state of every scene that differs from a rebuild, and checks `frame`.
    #[cold]
    fn verify_scenes(&mut self, frame: &Frame) {
        self.mismatches += self
            .clients
            .values()
            .map(|client| verify::mismatches(&client.scene))
            .sum::<u64>();
        self.check(frame);
    }
}

/// Warns that a frame has `runs` glyph runs and the compositor no glyph rasteriser.
#[cold]
fn warn_unrasterised(runs: u32) {
    tracing::warn!(
        runs,
        "a frame has glyph runs and the compositor has no glyph rasteriser, so they draw nothing"
    );
}
