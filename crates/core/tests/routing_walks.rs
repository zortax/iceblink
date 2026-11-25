//! The paint-order walks that routing a gesture makes.
//!
//! Steps over one point walk again only after a commit that can change where they route from.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::{
    ChunkKey, Color, GesturePhase, OrderEntry, OrderId, OrderNode, Paint, PaintId, PaintKind,
    ScrollBehaviour, ScrollGesture, ScrollId, ScrollNode, ScrollPhysics, ScrollRouting,
    ScrollSource, SnapKind, Sources, Transaction, TransformId, TransformNode,
};

use common::{Scene, identity, props, quad, stat};

/// The number of rows the sidebar holds at a time.
const ROWS: u32 = 20;

/// A page port beside a sidebar port of rows, over a background.
struct Pages {
    scene: Scene,
    white: PaintId,
    page: TransformId,
    sidebar: TransformId,
    rows: OrderId,
    /// The key of the sidebar's first row.
    first: u32,
}

/// Returns an instant, vertically scrollable behaviour.
fn behaviour() -> ScrollBehaviour {
    ScrollBehaviour {
        physics: ScrollPhysics::Instant,
        snap: Arc::from([]),
        snap_kind: SnapKind::Mandatory,
        bounce: Sources::NONE,
        user_scrollable: [false, true],
        chain: Sources::NONE,
    }
}

/// Creates a scroll node over `port` in `space` and the transform node of its content.
fn port(tx: &mut Transaction, space: TransformId, port: [f32; 4]) -> (ScrollId, TransformId) {
    let node = tx.create_scroll(ScrollNode {
        space,
        port,
        content: [port[2], 2000.0],
        offset: [0.0, 0.0],
        behaviour: Some(behaviour()),
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(node),
        ..identity(Some(space))
    });
    (node, content)
}

impl Pages {
    /// Builds the background, a sidebar of rows over the left fifth, and a page over the rest
    /// whose content draws nothing.
    fn new() -> Self {
        let mut scene = Scene::new();
        let mut tx = scene.tx();
        let white = tx.create_paint(Paint::Solid(Color([1.0; 4])));

        let background = ChunkKey::new(1, 0);
        let mut chunk = tx.record(background, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, white);
        chunk.quad(quad([0.0, 0.0, 200.0, 200.0], fill));

        let (_, sidebar) = port(&mut tx, scene.space, [0.0, 0.0, 40.0, 200.0]);
        let (_, page) = port(&mut tx, scene.space, [40.0, 0.0, 160.0, 200.0]);
        let rows = tx.create_order(OrderNode::default());
        tx.splice(
            scene.root,
            0,
            0,
            &[OrderEntry::Chunk(background), OrderEntry::Node(rows)],
        );
        for row in 0..ROWS {
            record_row(&mut tx, sidebar, white, row);
            tx.splice(rows, row, 0, &[OrderEntry::Chunk(ChunkKey::new(2, row))]);
        }
        scene.commit(tx);

        Self {
            scene,
            white,
            page,
            sidebar,
            rows,
            first: 0,
        }
    }

    /// Hands a wheel step over the page, and returns whether a node took it.
    fn step(&mut self) -> bool {
        let routing = self.scene.compositor.scroll_input(
            self.scene.output,
            ScrollGesture {
                position: [100.0, 100.0],
                delta: [0.0, 40.0],
                phase: GesturePhase::Step,
                source: ScrollSource::Wheel,
                time_ns: 0,
            },
        );
        matches!(routing, ScrollRouting::Handled { .. })
    }

    /// Commits the sidebar's first row removed and a new row recorded after its last.
    fn recycle(&mut self) {
        let mut tx = self.scene.tx();
        tx.splice(self.rows, 0, 1, &[]);
        tx.remove_chunk(ChunkKey::new(2, self.first));

        let row = self.first + ROWS;
        record_row(&mut tx, self.sidebar, self.white, row);
        tx.splice(
            self.rows,
            ROWS - 1,
            0,
            &[OrderEntry::Chunk(ChunkKey::new(2, row))],
        );
        self.first += 1;

        self.scene.commit(tx);
    }

    /// Returns how many paint-order walks routing has made.
    fn walks(&self) -> u64 {
        stat(&self.scene.compositor, "work.routing_walks")
    }
}

/// Records row `row` of the sidebar in `space`.
fn record_row(tx: &mut Transaction, space: TransformId, paint: PaintId, row: u32) {
    let mut chunk = tx.record(ChunkKey::new(2, row), props(space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, row as f32 * 20.0, 40.0, 18.0], fill));
}

/// Steps over a port walk the paint order only when a chunk draws in the port, once, and again
/// only after a commit that changes what draws in the port or the hit.
#[test]
fn a_second_step_over_the_same_point_walks_nothing() {
    let mut pages = Pages::new();
    assert!(pages.step());
    assert!(pages.step());
    assert_eq!(pages.walks(), 0, "a port no chunk draws in needs no walk");

    pages.recycle();
    assert!(pages.step());
    assert_eq!(
        pages.walks(),
        0,
        "recycled rows beside the port add no walk"
    );

    let mut tx = pages.scene.tx();
    let key = ChunkKey::new(3, 0);
    let mut chunk = tx.record(key, props(pages.page));
    let fill = chunk.paint_ref(PaintKind::SOLID, pages.white);
    chunk.quad(quad([40.0, 0.0, 160.0, 20.0], fill));
    tx.splice(pages.scene.root, 2, 0, &[OrderEntry::Chunk(key)]);
    pages.scene.commit(tx);
    assert!(pages.step());
    assert_eq!(
        pages.walks(),
        1,
        "the page's first chunk makes routing walk"
    );
    assert!(pages.step());
    pages.recycle();
    assert!(pages.step());

    let mut tx = pages.scene.tx();
    tx.set_paint(pages.white, Paint::Solid(Color([0.9, 0.9, 0.9, 1.0])));
    pages.scene.commit(tx);
    assert!(pages.step());
    assert_eq!(
        pages.walks(),
        1,
        "steps over the same point reuse the start across recycled rows and paint writes"
    );

    let mut tx = pages.scene.tx();
    let mut chunk = tx.record(key, props(pages.page));
    let fill = chunk.paint_ref(PaintKind::SOLID, pages.white);
    chunk.quad(quad([40.0, 20.0, 160.0, 20.0], fill));
    pages.scene.commit(tx);
    assert!(pages.step());
    assert_eq!(
        pages.walks(),
        2,
        "a chunk recorded in the page makes routing walk again"
    );
}
