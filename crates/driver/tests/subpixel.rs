//! An output's subpixel stripe order is passed to its frame requests and its renderer.

use std::any::Any;
use std::sync::{Arc, Mutex};

use iceblink::smallvec::SmallVec;
use iceblink::{
    BufferAge, Built, ClientId, CommitError, CompositorEvent, Counters, Frame, FrameClock, FrameId,
    FrameOutcome, FrameRequest, HitResult, IdSpace, MemoryReport, OrderNode, OutputId,
    RenderCapabilities, RenderTarget, ScrollGesture, ScrollRouting, SubpixelOrder, Transaction,
};
use iceblink::{
    CommitReport, Compositor, CompositorBudgets, CompositorCapabilities, CompositorClient,
    CompositorMemory, FontRegistry, GlyphRaster, OutputConfig, Renderer,
};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::fakes::NullRenderer;

/// The stripe orders the frame requests and the renderer were given, oldest first.
#[derive(Default)]
struct Seen {
    requests: Vec<Option<SubpixelOrder>>,
    renderer: Vec<SubpixelOrder>,
}

type Shared = Arc<Mutex<Seen>>;

/// The reference compositor, noting the stripe order of every frame request.
struct Noting {
    inner: CompositorImpl,
    seen: Shared,
}

impl Compositor for Noting {
    fn capabilities(&self) -> CompositorCapabilities {
        self.inner.capabilities()
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        self.inner.register_client()
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.inner.unregister_client(client);
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        self.inner.transaction(client)
    }

    fn commit(&mut self, transaction: Transaction) -> Result<CommitReport, CommitError> {
        self.inner.commit(transaction)
    }

    fn use_font_registry(&mut self, registry: Arc<dyn FontRegistry>) {
        self.inner.use_font_registry(registry);
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        self.inner.font_registry()
    }

    fn use_glyph_raster(&mut self, raster: Arc<dyn GlyphRaster>) {
        self.inner.use_glyph_raster(raster);
    }

    fn create_output(&mut self, config: OutputConfig) -> OutputId {
        self.inner.create_output(config)
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.inner.configure_output(output, config);
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.inner.destroy_output(output);
    }

    fn build_frame(
        &mut self,
        output: OutputId,
        request: &FrameRequest<'_>,
        into: &mut Frame,
    ) -> Built {
        self.seen.lock().unwrap().requests.push(request.subpixel);
        self.inner.build_frame(output, request, into)
    }

    fn frame_done(&mut self, output: OutputId, frame: FrameId, outcome: &FrameOutcome) {
        self.inner.frame_done(output, frame, outcome);
    }

    fn scroll_input(&mut self, output: OutputId, gesture: ScrollGesture) -> ScrollRouting {
        self.inner.scroll_input(output, gesture)
    }

    fn hit_test(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.inner.hit_test(output, point)
    }

    fn drain_events(&mut self, out: &mut Vec<CompositorEvent>) {
        self.inner.drain_events(out);
    }

    fn set_verification(&mut self, on: bool) {
        self.inner.set_verification(on);
    }

    fn counters(&self) -> &Counters {
        self.inner.counters()
    }

    fn memory(&self) -> CompositorMemory {
        self.inner.memory()
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.inner.set_budgets(budgets);
    }

    fn budgets(&self) -> CompositorBudgets {
        self.inner.budgets()
    }

    fn trim(&mut self, to_bytes: u64) -> u64 {
        self.inner.trim(to_bytes)
    }

    fn forget_caches(&mut self) -> u64 {
        self.inner.forget_caches()
    }
}

/// A null renderer, noting every stripe order it is given.
struct Striped {
    inner: NullRenderer,
    seen: Shared,
}

impl Renderer for Striped {
    fn capabilities(&self) -> RenderCapabilities {
        self.inner.capabilities()
    }

    fn configure(&mut self, target: RenderTarget) {
        self.inner.configure(target);
    }

    fn target(&self) -> Option<RenderTarget> {
        self.inner.target()
    }

    fn target_age(&self) -> BufferAge {
        self.inner.target_age()
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        self.inner.draw(frame)
    }

    fn set_subpixel_order(&mut self, order: SubpixelOrder) {
        self.seen.lock().unwrap().renderer.push(order);
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }

    fn as_any(&self) -> Option<&dyn Any> {
        None
    }
}

#[test]
fn the_stripe_order_reaches_the_frame_requests_and_the_renderer() {
    let seen = Shared::default();
    let compositor = Noting {
        inner: CompositorImpl::new(),
        seen: Arc::clone(&seen),
    };
    let mut driver = InProcess::new(Box::new(compositor), DriverOptions::stepped());
    let (client, _) = driver.register_client();
    let config = OutputConfig {
        size: [64, 64],
        scale: 1.0,
        opaque: true,
    };
    let striped = Arc::clone(&seen);
    let output = driver.create_output(
        config,
        Box::new(move || {
            Ok(Box::new(Striped {
                inner: NullRenderer::new(),
                seen: striped,
            }))
        }),
    );
    driver.set_subpixel_order(output, Some(SubpixelOrder::BlueToRed));
    let mut tx = driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    driver.commit(tx);
    driver.step(FrameClock {
        now_ns: 0,
        present_ns: 16_666_667,
        interval_ns: 16_666_667,
    });

    let seen = seen.lock().unwrap();
    assert_eq!(seen.renderer, [SubpixelOrder::BlueToRed]);
    assert_eq!(seen.requests, [Some(SubpixelOrder::BlueToRed)]);
}
