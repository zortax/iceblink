//! Building a frame for an output.

use iceblink::damage::DamageSet;
use iceblink::{
    Built, ChunkNote, Frame, FrameDamage, FrameId, FrameRequest, OutputId, RenderCapabilities,
    ShaderClock,
};

use crate::naive::compositor::NaiveCompositor;
use crate::naive::frame::{self, Input};
use crate::naive::output::Output;

impl NaiveCompositor {
    /// Builds the next frame of `output`, or returns `Built::Idle` when the output owes none or
    /// does not exist.
    ///
    /// A frame is owed in these cases:
    ///
    /// - After any change.
    /// - After a frame whose damage did not retire.
    /// - After a frame that drew with a shader reading the clock.
    /// - When the renderer's capabilities or the stripe order change.
    /// - When the route history of a frame path is forgotten.
    ///
    /// A built frame damages the whole output. Every call counts as a frame for the route
    /// histories of path masks, whether it builds one or not. While a frame path keeps its
    /// history, `Built::Idle` asks for the next frame.
    pub(super) fn build(
        &mut self,
        output: OutputId,
        request: &FrameRequest<'_>,
        into: &mut Frame,
    ) -> Built {
        if self.content.routes.next_frame() {
            self.changed();
        }
        let start = *self.started.get_or_insert(request.clock.now_ns);
        self.fit_pages(request.caps.max_texture_size);

        let Some(state) = self.outputs.get_mut(&output) else {
            return Built::Idle { wake_ns: None };
        };

        let built_for = (*request.caps, request.subpixel);
        if state.built_for != Some(built_for) || state.reads_time {
            state.owes_frame = true;
        }
        if !state.owes_frame {
            let clock = request.clock;
            let wake_ns = self
                .content
                .routes
                .waiting()
                .then(|| clock.present_ns.saturating_add(clock.interval_ns.max(1)));
            return Built::Idle { wake_ns };
        }

        state.owes_frame = false;
        state.built_for = Some(built_for);
        start_frame(state, output, request, start, into);
        state
            .shaders
            .destroy_removed(&self.scene, &mut into.resources.shaders);

        self.drop_stale_content();

        let state = self.outputs.get_mut(&output).expect("looked up above");
        let lowered = frame::lower(
            Input {
                scene: &self.scene,
                root: self.scene.roots.get(&output).copied(),
                config: state.config,
                caps: RenderCapabilities {
                    max_texture_size: self.page_limit,
                    ..*request.caps
                },
                subpixel: request.subpixel,
                raster: self.raster.as_deref(),
                fonts: self.fonts.as_ref(),
                content: &mut self.content,
                slots: &mut state.slots,
                shaders: &mut state.shaders,
            },
            into,
        );

        self.content
            .sync(&mut state.pages, &mut into.resources.pages);
        self.builds += 1;
        self.content.drawn(&lowered.pages, self.builds);
        state.drawn_pages = lowered.pages;
        state.reads_time = lowered.reads_time;

        let mut counters = lowered.counters;
        counters.damage_px = u64::from(state.config.size[0]) * u64::from(state.config.size[1]);
        self.counters.frame = counters;
        Built::Frame
    }

    /// Lowers the page limit to `max_texture_size` when a renderer's textures are smaller.
    fn fit_pages(&mut self, max_texture_size: u32) {
        if max_texture_size >= self.page_limit {
            return;
        }

        // Pages serve every output, so pages made for a larger limit are replaced.
        if self.page_limit != u32::MAX {
            self.drop_caches();
        }
        self.page_limit = max_texture_size;
    }

    /// Destroys the pages of removed or rewritten images, then evicts pages over the atlas
    /// budget.
    fn drop_stale_content(&mut self) {
        let clients = &self.scene.clients;
        self.content
            .images
            .drop_missing(&mut self.content.pages, |(client, slot), version| {
                clients
                    .get(&client)
                    .and_then(|client| client.images.at(slot))
                    .is_some_and(|image| image.version == version)
            });

        if let Some(limit) = self.budgets.atlas_bytes
            && self.content.pages.bytes() > limit
        {
            let pinned = self.pinned();
            self.content.evict(limit, &pinned);
        }
    }
}

/// Starts frame `into` of `output` with a new id, the output's configuration, the shader clock
/// and full damage, and drops the renderer's resident chunks when a reset is owed.
fn start_frame(
    state: &mut Output,
    output: OutputId,
    request: &FrameRequest<'_>,
    start: u64,
    into: &mut Frame,
) {
    state.last_frame += 1;
    let id = FrameId(state.last_frame);
    state.pending = Some(id);

    into.clear();
    into.id = id;
    into.output = output;
    into.size = state.config.size;
    into.scale = state.config.scale;
    into.opaque = state.config.opaque;
    into.clock = request.clock;
    into.shader_clock = shader_clock(start, state.last_present, request.clock.present_ns);
    state.last_present = Some(request.clock.present_ns);
    into.damage = FrameDamage {
        draw: DamageSet::full(),
        present: DamageSet::full(),
        driven: DamageSet::new(),
    };

    if state.reset_resident && request.caps.resident_chunks {
        into.chunk_notes.push(ChunkNote::ResetAll);
    }
    state.reset_resident = false;
}

/// Returns the shader times of a frame presented at `present`, with the compositor started at
/// `start` and the output's previous frame presented at `previous`.
fn shader_clock(start: u64, previous: Option<u64>, present: u64) -> ShaderClock {
    let seconds = |from: u64, to: u64| ((i128::from(to) - i128::from(from)) as f64 / 1e9) as f32;
    ShaderClock {
        time: seconds(start, present),
        delta: previous.map_or(0.0, |previous| seconds(previous, present)),
    }
}
