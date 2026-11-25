//! Every trait of the contract can be held as a trait object.

use std::sync::Arc;

use iceblink::{
    BufferAge, Frame, FrameOutcome, MemoryReport, RenderCapabilities, RenderTarget, SkipReason,
};
use iceblink::{
    Compositor, CompositorClient, FontRegistry, GlyphRaster, Presenter, Renderer, RendererFactory,
};

#[test]
fn every_trait_is_object_safe_and_the_shared_ones_cross_threads() {
    fn send<T: Send + ?Sized>() {}
    fn shared<T: Send + Sync + ?Sized>() {}

    let compositor: Option<Box<dyn Compositor>> = None;
    let client: Option<Box<dyn CompositorClient>> = None;
    let renderer: Option<Box<dyn Renderer>> = None;
    let presenter: Option<Box<dyn Presenter>> = None;
    let raster: Option<Arc<dyn GlyphRaster>> = None;
    let fonts: Option<Arc<dyn FontRegistry>> = None;
    send::<Box<dyn Compositor>>();
    send::<RendererFactory>();
    shared::<Arc<dyn GlyphRaster>>();
    shared::<Arc<dyn FontRegistry>>();

    assert!(compositor.is_none() && client.is_none());
    assert!(renderer.is_none() && presenter.is_none());
    assert!(raster.is_none() && fonts.is_none());
}

/// A renderer that holds one pooled target and draws nothing.
struct Pooled {
    held: u64,
}

impl Renderer for Pooled {
    fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities::MINIMAL
    }

    fn configure(&mut self, _target: RenderTarget) {}

    fn target(&self) -> Option<RenderTarget> {
        None
    }

    fn target_age(&self) -> BufferAge {
        BufferAge::Unknown
    }

    fn draw(&mut self, _frame: &Frame) -> FrameOutcome {
        FrameOutcome::Skipped(SkipReason::Unconfigured)
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport::default()
    }

    fn release_cached_targets(&mut self) -> u64 {
        core::mem::take(&mut self.held)
    }
}

#[test]
fn an_idle_release_frees_the_cached_targets_unless_a_renderer_says_otherwise() {
    let mut renderer: Box<dyn Renderer> = Box::new(Pooled { held: 64 });

    assert_eq!(renderer.release_idle_resources(), 64);
    assert_eq!(renderer.release_idle_resources(), 0);
    assert_eq!(
        renderer.present_held(),
        FrameOutcome::Skipped(SkipReason::Undamaged)
    );
    assert!(renderer.presenter().is_none());
}
