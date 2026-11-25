//! Two outputs of one compositor drawn by two renderers on one shared device.

mod common;

use std::sync::Arc;

use iceblink::{
    Built, ChunkKey, Frame, FrameClock, FrameOutcome, FrameRequest, OrderEntry, OrderId, OrderNode,
    OutputId, PaintKind, RenderTarget,
};
use iceblink::{Compositor, OutputConfig, Renderer};
use iceblink_testing::FrameValidator;
use iceblink_testing::gpu::{device_lock, skip};
use iceblink_testing::validate::describe;
use iceblink_wgpu::vector_coverage::CoverageRaster;
use iceblink_wgpu::{SharedGraphics, WgpuRenderer, wgpu};

use common::slice::{SIDE, Slice};
use common::text::run;
use common::{props, stat};

/// Returns a renderer on `graphics` drawing into an offscreen image of the slice's side.
fn renderer(graphics: &SharedGraphics) -> Option<WgpuRenderer> {
    match graphics.renderer_offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm) {
        Ok(mut renderer) => {
            eprintln!("adapter: {}", renderer.gpu().describe());
            let gpu = renderer.gpu().clone();
            let paths = CoverageRaster::new(gpu.device(), gpu.queue(), SIDE, SIDE);
            renderer.set_vector_raster(Box::new(paths), "coverage");
            Some(renderer)
        }
        Err(failure) => skip(&format!("no usable graphics device: {failure}")),
    }
}

/// An output of the slice's compositor besides its own, with the renderer that draws it.
struct Second {
    output: OutputId,
    renderer: WgpuRenderer,
    validator: FrameValidator,
    frame: Frame,
}

impl Second {
    /// Builds, checks and draws the output's next frame, and returns whether one was built.
    fn draw(&mut self, slice: &mut Slice<WgpuRenderer>) -> bool {
        let caps = self.renderer.capabilities();
        let request = FrameRequest {
            caps: &caps,
            age: self.renderer.target_age(),
            clock: FrameClock::default(),
            subpixel: None,
        };
        let built = slice
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built != Built::Frame {
            return false;
        }
        if let Err(violations) = self.validator.check(&self.frame) {
            panic!(
                "the frame breaks the frame contract:\n{}",
                describe(&violations)
            );
        }
        let outcome = self.renderer.draw(&self.frame);
        assert!(
            matches!(outcome, FrameOutcome::Presented(_)),
            "the second output presents: {outcome:?}"
        );
        slice
            .compositor
            .frame_done(self.output, self.frame.id, &outcome);
        true
    }
}

/// Returns the largest channel difference between the images the two renderers presented last.
fn difference(first: &WgpuRenderer, second: &WgpuRenderer) -> u8 {
    let read = |renderer: &WgpuRenderer| {
        renderer
            .read_presented()
            .expect("an offscreen image can be read back")
    };
    read(first).max_difference(&read(second))
}

/// Records chunk `owner` with one line of `text` at height `y` and appends it to `root` at `at`.
fn line(
    tx: &mut iceblink::Transaction,
    world: &common::story::World,
    root: OrderId,
    at: u32,
    owner: u64,
    text: &str,
    y: f32,
) {
    let key = ChunkKey::new(owner, 0);
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, world.paints[0]);
    run(&mut chunk, world.font, text, 14.0, [4.0, y], fill);
    tx.splice(root, at, 0, &[OrderEntry::Chunk(key)]);
}

#[test]
fn two_outputs_on_one_device_draw_the_same_glyphs_rasterised_once() {
    let _device = device_lock();
    let graphics = SharedGraphics::new();
    let Some(first) = renderer(&graphics) else {
        return;
    };
    let mut second = renderer(&graphics).expect("the device opened before");
    assert!(
        Arc::ptr_eq(first.gpu(), second.gpu()),
        "both renderers draw on one device"
    );

    let mut slice = Slice::new(first);
    let mut tx = slice.tx();
    slice.world.solid(&mut tx, [0.1, 0.2, 0.6, 1.0]);
    let first_root = slice.world.root;
    line(
        &mut tx,
        &slice.world,
        first_root,
        0,
        1,
        "Shared glyphs",
        20.0,
    );
    slice.compositor.commit(tx).expect("the first line commits");
    assert!(slice.draw());
    let rasterised = stat(&slice.compositor, "content.glyphs_rasterised");
    assert!(rasterised > 0);

    // The second output joins after the first has drawn, and shows the same line.
    let config = OutputConfig {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    };
    let output = slice.compositor.create_output(config);
    second.configure(RenderTarget {
        size: config.size,
        scale: config.scale,
        opaque: config.opaque,
    });
    let mut tx = slice.tx();
    let second_root = tx.create_order(OrderNode::default());
    line(
        &mut tx,
        &slice.world,
        second_root,
        0,
        2,
        "Shared glyphs",
        20.0,
    );
    tx.set_root(output, Some(second_root));
    slice
        .compositor
        .commit(tx)
        .expect("the second root commits");
    let mut validator = FrameValidator::new();
    validator.set_capabilities(Some(second.capabilities()));
    let mut second = Second {
        output,
        renderer: second,
        validator,
        frame: Frame::default(),
    };
    assert!(second.draw(&mut slice));
    assert_eq!(
        stat(&slice.compositor, "content.glyphs_rasterised"),
        rasterised,
        "the second output draws the glyphs rasterised for the first"
    );
    assert_eq!(difference(&slice.renderer, &second.renderer), 0);

    // A line of new glyphs on both outputs is rasterised once.
    let mut tx = slice.tx();
    line(&mut tx, &slice.world, first_root, 1, 3, "Quartz jinx", 60.0);
    line(
        &mut tx,
        &slice.world,
        second_root,
        1,
        4,
        "Quartz jinx",
        60.0,
    );
    slice.compositor.commit(tx).expect("the new lines commit");
    let added = stat(&slice.compositor, "content.glyphs_rasterised");
    assert!(
        added > rasterised,
        "the new glyphs are rasterised at commit"
    );
    assert!(slice.draw());
    assert!(second.draw(&mut slice));
    assert_eq!(
        stat(&slice.compositor, "content.glyphs_rasterised"),
        added,
        "neither output rasterises them again"
    );
    assert_eq!(difference(&slice.renderer, &second.renderer), 0);
}
