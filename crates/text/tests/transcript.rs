//! A recorded paragraph, drawn by the reference compositor into a capture renderer.

mod common;

use std::path::Path;
use std::sync::Arc;

use iceblink::{
    Anchor, Built, ChunkFlags, ChunkKey, ChunkProps, Color, Frame, FrameClock, FrameRequest,
    Matrix4, OrderEntry, OrderNode, Paint, PaintKind, RenderTarget, TransformNode,
};
use iceblink::{Compositor as _, OutputConfig, Renderer as _};
use iceblink_core::CompositorImpl;
use iceblink_testing::assert_valid_frame;
use iceblink_testing::capture::CaptureRenderer;
use iceblink_testing::golden::assert_text;
use iceblink_text::SwashRaster;
use iceblink_text::typeset::{Alignment, FontStyle, Position, Property, TextPaint, TextRecorder};

use crate::common::typeset::{Fonts, SANS};

const SIZE: [u32; 2] = [240, 120];

const TEXT: &str = "A bold, slanted and underlined paragraph, struck through in red.";

/// Returns what a capture renderer draws of the paragraph's first frame.
fn drawn_paragraph() -> String {
    let mut fonts = Fonts::new();
    let mut text = fonts.typesetter.text(TEXT);
    text.style(Property::Family(SANS))
        .style(Property::Size(15.0))
        .span(2..6, Property::Weight(700.0))
        .span(8..15, Property::Style(FontStyle::Italic))
        .span(20..30, Property::Underline(true))
        .span(42..63, Property::LineThrough(true))
        .span(42..63, Property::Paint(TextPaint::Color([200, 0, 0, 255])));
    let layout = text.build(Some(SIZE[0] as f32 - 20.0), Alignment::Start);

    let mut compositor = CompositorImpl::new();
    compositor.use_font_registry(Arc::clone(fonts.typesetter.registry()) as _);
    compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
    let (client, _) = compositor.register_client();
    let config = OutputConfig {
        size: SIZE,
        scale: 1.0,
        opaque: true,
    };
    let output = compositor.create_output(config);

    let mut tx = compositor.transaction(client);
    let space = tx.create_transform(TransformNode {
        parent: None,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    });
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    let highlight = tx.create_paint(Paint::Solid(Color([0.7, 0.8, 1.0, 1.0])));
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(
        key,
        ChunkProps {
            space,
            clip: None,
            flags: ChunkFlags::default(),
        },
    );
    let mut recorder = TextRecorder::new(&mut chunk);
    let origin = [10.0, 8.0];
    let highlight = TextPaint::Slot(PaintKind::SOLID, highlight);
    recorder.selection(&layout, 8..30, origin, highlight);
    recorder.layout(&layout, origin);
    recorder.caret(
        &layout,
        Position::new(30),
        1.0,
        origin,
        TextPaint::default(),
    );
    tx.splice(root, 0, 0, &[OrderEntry::Chunk(key)]);
    compositor.commit(tx).expect("the paragraph commits");

    let mut renderer = CaptureRenderer::new();
    renderer.configure(RenderTarget {
        size: config.size,
        scale: config.scale,
        opaque: config.opaque,
    });
    let caps = renderer.capabilities();
    let request = FrameRequest {
        caps: &caps,
        age: renderer.target_age(),
        clock: FrameClock::default(),
        subpixel: None,
    };
    let mut frame = Frame::default();
    assert_eq!(
        compositor.build_frame(output, &request, &mut frame),
        Built::Frame
    );
    assert_valid_frame(&frame);
    let outcome = renderer.draw(&frame);
    compositor.frame_done(output, frame.id, &outcome);
    let captured = renderer.last().expect("a frame was drawn");
    captured.drawing.as_str().to_owned()
}

#[test]
fn a_recorded_paragraph_draws_as_its_golden() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/paragraph.txt");
    assert_text(&golden, &drawn_paragraph());
}
