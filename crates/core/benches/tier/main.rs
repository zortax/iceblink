//! Routing wheel steps over a page whose content draws nothing, beside a sidebar of many rows,
//! alone and after commits that recycle a row of the sidebar.
//!
//! With `TIER_STEPS` set, the binary runs that many rounds and exits without measuring, so a
//! counter such as `perf stat -e instructions:u` can take the difference between two counts.
//! `TIER_ROUND` picks the round: `step` (the default), `recycle` for a recycling commit, or
//! `both` for a recycling commit and then a step.

use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group};
use iceblink::{
    Anchor, BorderStyle, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Color, GesturePhase,
    Matrix4, OrderEntry, OrderId, OrderNode, OutputId, Paint, PaintKind, PaintRef, Quad, ROUND,
    ScrollBehaviour, ScrollGesture, ScrollNode, ScrollPhysics, ScrollRouting, ScrollSource,
    SnapKind, Sources, SpaceRef, Transaction, TransformId, TransformNode,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;

/// The output's size in pixels.
const SIDE: [u32; 2] = [1024, 768];

/// The number of rows of the sidebar, each a chunk of its own.
const ROWS: u32 = 10_000;

/// The sidebar's width, and where the page starts.
const SIDEBAR: f32 = 200.0;

/// Where the steps happen, over the page.
const AT: [f32; 2] = [600.0, 400.0];

/// A compositor drawing a background, a sidebar of rows and a page that draws nothing.
struct Pages {
    compositor: CompositorImpl,
    client: ClientId,
    output: OutputId,
    sidebar: Sidebar,
    /// The direction of the next step.
    down: bool,
}

/// The rows of the sidebar, which recycling commits replace one at a time.
struct Sidebar {
    rows: OrderId,
    space: TransformId,
    /// The key of the first row the order node holds.
    first: u32,
}

impl Pages {
    fn new() -> Self {
        let mut compositor = CompositorImpl::new();
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: SIDE,
            scale: 1.0,
            opaque: true,
        });

        let mut tx = compositor.transaction(client);
        let sidebar = scene(&mut tx, output);
        compositor.commit(tx).expect("the scene commits");

        Self {
            compositor,
            client,
            output,
            sidebar,
            down: true,
        }
    }

    /// Commits the sidebar's first row removed and a new row recorded after its last.
    fn recycle(&mut self) {
        let sidebar = &mut self.sidebar;
        let mut tx = self.compositor.transaction(self.client);
        tx.splice(sidebar.rows, 0, 1, &[]);
        tx.remove_chunk(ChunkKey::new(2, sidebar.first));

        let row = sidebar.first + ROWS;
        let key = ChunkKey::new(2, row);
        record_row(&mut tx, key, sidebar.space, row);
        tx.splice(sidebar.rows, ROWS - 1, 0, &[OrderEntry::Chunk(key)]);
        sidebar.first += 1;

        self.compositor
            .commit(tx)
            .expect("the recycled row commits");
    }

    /// Hands one wheel step over the page, alternately down and up.
    fn step(&mut self) -> ScrollRouting {
        let delta = if self.down { 40.0 } else { -40.0 };
        self.down = !self.down;
        self.compositor.scroll_input(
            self.output,
            ScrollGesture {
                position: AT,
                delta: [0.0, delta],
                phase: GesturePhase::Step,
                source: ScrollSource::Wheel,
                time_ns: 0,
            },
        )
    }
}

/// Builds the background, the sidebar and the page as the content of `output`, and returns the
/// sidebar.
fn scene(tx: &mut Transaction, output: OutputId) -> Sidebar {
    let [width, height] = SIDE.map(|side| side as f32);
    let space = tx.create_transform(transform(None, None));
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    let white = tx.create_paint(Paint::Solid(Color([1.0; 4])));

    let background = ChunkKey::new(1, 0);
    let mut chunk = tx.record(background, props(space));
    let fill = chunk.paint_ref(PaintKind::SOLID, white);
    chunk.quad(quad([0.0, 0.0, width, height], fill));
    tx.splice(root, 0, 0, &[OrderEntry::Chunk(background)]);

    let sidebar = tx.create_scroll(scroll(
        space,
        [0.0, 0.0, SIDEBAR, height],
        ROWS as f32 * 20.0,
    ));
    let rows_space = tx.create_transform(transform(Some(space), Some(sidebar)));
    let rows = tx.create_order(OrderNode::default());
    tx.splice(root, 1, 0, &[OrderEntry::Node(rows)]);
    for row in 0..ROWS {
        let key = ChunkKey::new(2, row);
        record_row(tx, key, rows_space, row);
        tx.splice(rows, row, 0, &[OrderEntry::Chunk(key)]);
    }

    let page = tx.create_scroll(scroll(
        space,
        [SIDEBAR, 0.0, width - SIDEBAR, height],
        4.0 * height,
    ));
    tx.create_transform(transform(Some(space), Some(page)));

    Sidebar {
        rows,
        space: rows_space,
        first: 0,
    }
}

/// Records row `row` of the sidebar under `key`.
fn record_row(tx: &mut Transaction, key: ChunkKey, space: TransformId, row: u32) {
    let mut chunk = tx.record(key, props(space));
    chunk.quad(quad(
        [0.0, row as f32 * 20.0, SIDEBAR, 18.0],
        PaintRef::color([200, 200, 200, 255]),
    ));
}

/// Returns a transform node with an identity matrix under `parent`, scrolled by `scroll`.
fn transform(parent: Option<TransformId>, scroll: Option<iceblink::ScrollId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll,
        anchor: Anchor::Scrolls,
    }
}

/// Returns an instantly scrolling node over `port` with content `height` tall.
fn scroll(space: TransformId, port: [f32; 4], height: f32) -> ScrollNode {
    ScrollNode {
        space,
        port,
        content: [port[2], height],
        offset: [0.0, 0.0],
        behaviour: Some(ScrollBehaviour {
            physics: ScrollPhysics::Instant,
            snap: Arc::from([]),
            snap_kind: SnapKind::Mandatory,
            bounce: Sources::NONE,
            user_scrollable: [false, true],
            chain: Sources::NONE,
        }),
    }
}

/// Returns chunk props in `space` with no clip.
fn props(space: TransformId) -> ChunkProps {
    ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    }
}

/// Returns a borderless quad over `bounds` filled with `fill`.
fn quad(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

fn benches(c: &mut Criterion) {
    let mut pages = Pages::new();
    assert!(matches!(pages.step(), ScrollRouting::Handled { .. }));
    c.bench_function("tier/step_over_an_empty_page", |b| {
        b.iter(|| black_box(pages.step()));
    });
    c.bench_function("tier/recycle_then_step", |b| {
        b.iter(|| {
            pages.recycle();
            black_box(pages.step())
        });
    });
}

criterion_group!(tier, benches);

fn main() {
    if let Some(steps) = std::env::var("TIER_STEPS")
        .ok()
        .and_then(|steps| steps.parse::<u32>().ok())
    {
        let round = std::env::var("TIER_ROUND").unwrap_or_default();
        let mut pages = Pages::new();
        for _ in 0..steps {
            match round.as_str() {
                "recycle" => pages.recycle(),
                "both" => {
                    pages.recycle();
                    black_box(pages.step());
                }
                _ => {
                    black_box(pages.step());
                }
            }
        }
        return;
    }

    tier();
    Criterion::default().configure_from_args().final_summary();
}
