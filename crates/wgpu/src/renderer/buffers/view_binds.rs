//! Bind groups over texture views, kept while frames use them.

/// Bind groups over texture views, by buffer generation, view and sampler.
///
/// A bind group stays through the frame after the last one that used it.
#[derive(Debug, Default)]
pub(super) struct ViewBinds {
    entries: Vec<ViewBind>,
    frame: u64,
}

#[derive(Debug)]
struct ViewBind {
    generation: u64,
    view: wgpu::TextureView,
    sampler: Option<wgpu::Sampler>,
    bind: wgpu::BindGroup,
    used: u64,
}

impl ViewBinds {
    /// Starts a frame, and drops the bind groups the frame before did not use.
    pub(super) fn begin_frame(&mut self) {
        let frame = self.frame;
        self.entries.retain(|entry| entry.used == frame);
        self.frame = frame.wrapping_add(1);
    }

    /// Returns the bind group kept for `generation`, `view` and `sampler`, marked as used.
    pub(super) fn find(
        &mut self,
        generation: u64,
        view: &wgpu::TextureView,
        sampler: Option<&wgpu::Sampler>,
    ) -> Option<wgpu::BindGroup> {
        let frame = self.frame;
        let entry = self.entries.iter_mut().find(|entry| {
            entry.generation == generation
                && entry.view == *view
                && entry.sampler.as_ref() == sampler
        })?;
        entry.used = frame;
        Some(entry.bind.clone())
    }

    /// Keeps `bind` for `generation`, `view` and `sampler`, and drops the bind groups of other
    /// generations.
    pub(super) fn keep(
        &mut self,
        generation: u64,
        view: &wgpu::TextureView,
        sampler: Option<&wgpu::Sampler>,
        bind: &wgpu::BindGroup,
    ) {
        // A bind group naming a buffer that has since been replaced can never be found again.
        self.entries.retain(|entry| entry.generation == generation);
        self.entries.push(ViewBind {
            generation,
            view: view.clone(),
            sampler: sampler.cloned(),
            bind: bind.clone(),
            used: self.frame,
        });
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}
