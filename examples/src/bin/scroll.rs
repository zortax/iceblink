//! A long list in a clipped scroll port, scrolled by the mouse wheel through `scroll_to`.

use iceblink::{
    Anchor, ChunkKey, ClipId, ClipNode, ClipShape, Matrix4, PaintId, PaintKind, ROUND, ScrollId,
    ScrollNode, Transaction, TransformId, TransformNode,
};
use iceblink_examples::{Demo, Line, Scene, quad, solid};

const BACKGROUND: ChunkKey = ChunkKey::new(1, 0);
const HEADER: ChunkKey = ChunkKey::new(2, 0);

/// The number of rows in the list.
const ROWS: usize = 200;

/// The number of rows one chunk holds.
const PER_CHUNK: usize = 20;

/// The height of one row.
const ROW: f32 = 36.0;

/// The distance of the port from the window's edges.
const MARGIN: f32 = 16.0;

/// The top of the port, below the header.
const TOP: f32 = 64.0;

struct List {
    paints: Paints,
    scroll: ScrollId,
    port: ClipId,
    content: TransformId,
    size: [f32; 2],
    offset: f32,
}

struct Paints {
    background: PaintId,
    port: PaintId,
    even: PaintId,
    odd: PaintId,
    ink: PaintId,
}

impl Demo for List {
    const TITLE: &'static str = "iceblink scroll";

    fn new(scene: &mut Scene, tx: &mut Transaction, size: [f32; 2]) -> Self {
        let paints = Paints {
            background: solid(tx, [0.94, 0.95, 0.97, 1.0]),
            port: solid(tx, [1.0, 1.0, 1.0, 1.0]),
            even: solid(tx, [0.9, 0.93, 0.99, 1.0]),
            odd: solid(tx, [0.98, 0.98, 1.0, 1.0]),
            ink: solid(tx, [0.1, 0.12, 0.18, 1.0]),
        };
        let space = scene.space();
        let scroll = tx.create_scroll(scroll_node(space, size, 0.0));
        let port = tx.create_clip(port_clip(space, size));
        let content = tx.create_transform(TransformNode {
            parent: Some(space),
            local: Matrix4::IDENTITY,
            scroll: Some(scroll),
            anchor: Anchor::Scrolls,
        });
        let list = Self {
            paints,
            scroll,
            port,
            content,
            size,
            offset: 0.0,
        };
        list.record_frame(scene, tx);
        scene.append(tx, BACKGROUND);
        list.record_header(scene, tx);
        scene.append(tx, HEADER);
        for chunk in 0..ROWS / PER_CHUNK {
            list.record_rows(scene, tx, chunk);
            scene.append(tx, rows_key(chunk));
        }
        list
    }

    fn resize(&mut self, scene: &Scene, tx: &mut Transaction, size: [f32; 2]) {
        self.size = size;
        self.offset = self.offset.min(largest_offset(size));
        let space = scene.space();
        tx.set_scroll_node(self.scroll, scroll_node(space, size, self.offset));
        tx.set_clip(self.port, port_clip(space, size));
        self.record_frame(scene, tx);
        for chunk in 0..ROWS / PER_CHUNK {
            self.record_rows(scene, tx, chunk);
        }
    }

    fn wheel(&mut self, tx: &mut Transaction, delta: [f32; 2]) {
        let offset = (self.offset + delta[1]).clamp(0.0, largest_offset(self.size));
        if offset != self.offset {
            self.offset = offset;
            tx.scroll_to(self.scroll, [0.0, offset]);
        }
    }
}

impl List {
    /// Records the background and the white face of the port.
    fn record_frame(&self, scene: &Scene, tx: &mut Transaction) {
        let mut chunk = tx.record(BACKGROUND, Scene::props(scene.space()));
        let background = chunk.paint_ref(PaintKind::SOLID, self.paints.background);
        chunk.quad(quad([0.0, 0.0, self.size[0], self.size[1]], background));
        // The port has square corners and one opaque quad behind it, so a scroll moves its pixels.
        let face = chunk.paint_ref(PaintKind::SOLID, self.paints.port);
        chunk.quad(quad(port(self.size), face));
    }

    fn record_header(&self, scene: &Scene, tx: &mut Transaction) {
        let mut chunk = tx.record(HEADER, Scene::props(scene.space()));
        let ink = chunk.paint_ref(PaintKind::SOLID, self.paints.ink);
        scene.font().record(
            &mut chunk,
            &Line {
                text: "A list of 200 rows, scrolled by the mouse wheel",
                size: 22.0,
                origin: [MARGIN, 42.0],
                paint: ink,
            },
        );
    }

    /// Records rows `chunk * PER_CHUNK ..` in the content space, clipped to the port.
    fn record_rows(&self, scene: &Scene, tx: &mut Transaction, chunk: usize) {
        let mut props = Scene::props(self.content);
        props.clip = Some(self.port);
        let mut recorder = tx.record(rows_key(chunk), props);
        let even = recorder.paint_ref(PaintKind::SOLID, self.paints.even);
        let odd = recorder.paint_ref(PaintKind::SOLID, self.paints.odd);
        let ink = recorder.paint_ref(PaintKind::SOLID, self.paints.ink);
        let [x, y, width, _] = port(self.size);
        for row in chunk * PER_CHUNK..(chunk + 1) * PER_CHUNK {
            let top = y + 4.0 + row as f32 * ROW;
            let fill = if row % 2 == 0 { even } else { odd };
            let mut band = quad([x + 4.0, top, width - 8.0, ROW - 4.0], fill);
            band.radii = [6.0; 8];
            recorder.quad(band);
            let label = format!("Row {}", row + 1);
            scene.font().record(
                &mut recorder,
                &Line {
                    text: &label,
                    size: 16.0,
                    origin: [x + 16.0, top + 22.0],
                    paint: ink,
                },
            );
        }
    }
}

fn rows_key(chunk: usize) -> ChunkKey {
    ChunkKey::new(100 + chunk as u64, 0)
}

/// Returns the port of a window of `size`.
fn port(size: [f32; 2]) -> [f32; 4] {
    [
        MARGIN,
        TOP,
        (size[0] - 2.0 * MARGIN).max(1.0),
        (size[1] - TOP - MARGIN).max(1.0),
    ]
}

/// Returns the height of the rows and the padding around them.
fn content_height() -> f32 {
    ROWS as f32 * ROW + 8.0
}

fn largest_offset(size: [f32; 2]) -> f32 {
    (content_height() - port(size)[3]).max(0.0)
}

fn scroll_node(space: TransformId, size: [f32; 2], offset: f32) -> ScrollNode {
    let port = port(size);
    ScrollNode {
        space,
        port,
        content: [port[2], content_height()],
        offset: [0.0, offset],
        behaviour: None,
    }
}

fn port_clip(space: TransformId, size: [f32; 2]) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: port(size),
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

fn main() {
    iceblink_examples::run::<List>();
}
