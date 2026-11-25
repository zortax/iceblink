//! A page that scrolls its root element, committed the way a toolkit commits it.

mod common;

use iceblink::Compositor;
use iceblink::{
    Anchor, Built, ChunkKey, ClipNode, ClipShape, Color, OrderEntry, OrderNode, Paint, PaintKind,
    PaintRef, ROUND, ScrollId, ScrollNode, Shift, TransformNode,
};

use common::{Scene, identity, props, quad, rect};

/// The width of the scrollbar gutter at the right edge of the output.
const GUTTER: f32 = 15.0;

/// A page node under the output's root, holding in order an opaque background over the whole
/// output, a scrollbar track and thumb in the gutter, the scrolled content, and an overlay node
/// with an empty chunk in a fixed space.
///
/// The port is the output less the gutter. The content holds rows of quads, clipped by the port.
struct Page {
    scene: Scene,
    scroll: ScrollId,
}

impl Page {
    fn new() -> Self {
        let mut scene = Scene::with_caps(common::FULL);
        let mut tx = scene.tx();
        let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));

        let width = 200.0 - GUTTER;
        let scroll = tx.create_scroll(ScrollNode {
            space: scene.space,
            port: [0.0, 0.0, width, 200.0],
            content: [width, 800.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(scene.space))
        });
        let fixed = tx.create_transform(TransformNode {
            anchor: Anchor::Fixed,
            ..identity(Some(content))
        });
        let port = tx.create_clip(ClipNode {
            parent: None,
            space: scene.space,
            shape: ClipShape::RoundedRect {
                rect: [0.0, 0.0, width, 200.0],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });

        let background = ChunkKey::new(1, 0);
        let mut chunk = tx.record(background, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([0.0, 0.0, 200.0, 200.0], fill));
        let track = ChunkKey::new(1, 1);
        let mut chunk = tx.record(track, props(scene.space));
        chunk.quad(quad([width, 0.0, GUTTER, 200.0], PaintRef::NONE));
        let thumb = ChunkKey::new(1, 2);
        let mut chunk = tx.record(thumb, props(scene.space));
        chunk.quad(quad([width + 3.0, 0.0, GUTTER - 6.0, 50.0], PaintRef::NONE));

        let rows = tx.create_order(OrderNode { effect: None });
        let mut entries = Vec::new();
        for row in 0..40 {
            let key = ChunkKey::new(2, row);
            let mut row_props = props(content);
            row_props.clip = Some(port);
            let mut chunk = tx.record(key, row_props);
            chunk.quad(quad([10.0, row as f32 * 20.0, 100.0, 12.0], PaintRef::NONE));
            entries.push(OrderEntry::Chunk(key));
        }
        tx.splice(rows, 0, 0, &entries);
        let scrolled = tx.create_order(OrderNode { effect: None });
        tx.splice(scrolled, 0, 0, &[OrderEntry::Node(rows)]);

        let overlay = tx.create_order(OrderNode { effect: None });
        let empty = ChunkKey::new(3, 0);
        tx.record(empty, props(fixed));
        tx.splice(overlay, 0, 0, &[OrderEntry::Chunk(empty)]);

        let page = tx.create_order(OrderNode { effect: None });
        tx.splice(
            page,
            0,
            0,
            &[
                OrderEntry::Chunk(background),
                OrderEntry::Chunk(track),
                OrderEntry::Chunk(thumb),
                OrderEntry::Node(scrolled),
                OrderEntry::Node(overlay),
            ],
        );
        tx.splice(scene.root, 0, 0, &[OrderEntry::Node(page)]);
        scene.commit(tx);
        scene.draw();

        Self { scene, scroll }
    }

    /// Scrolls to `offset` and moves the thumb with it, as one commit.
    fn scroll_to(&mut self, offset: f32) {
        let mut tx = self.scene.tx();
        tx.scroll_to(self.scroll, [0.0, offset]);
        tx.set_offset(ChunkKey::new(1, 2), [0.0, offset / 4.0]);
        self.scene.commit(tx);
    }
}

#[test]
fn a_root_port_backed_inside_the_page_node_shifts() {
    let mut page = Page::new();
    page.scroll_to(20.0);
    assert_eq!(page.scene.build(), Built::Frame);
    let region = rect(0, 0, 200 - GUTTER as i32, 200);
    let by = [0, -20];
    assert_eq!(page.scene.frame.shifts.as_slice(), &[Shift { region, by }]);
    assert_eq!(page.scene.compositor.counters().frame.shift_refusals, 0);
}
