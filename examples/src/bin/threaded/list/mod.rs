//! The recorded content: a header with a sliding bar, and a long list in a scroll port.

mod layout;
mod record;

use iceblink::{
    ChunkKey, ClientId, ClipId, HitResult, Matrix4, OrderEntry, OrderId, OrderNode, OutputId,
    PaintId, ScrollId, ScrollRouting, TransformId,
};
use iceblink::{ClientEvent, CompositorClient, ScrollWatch, WatchCause};
use iceblink_driver::thread::ClientHandle;
use iceblink_examples::{Font, solid};

use self::layout::{node, port, port_clip, rows_key, scaled, scroll_node};

const BACKGROUND: ChunkKey = ChunkKey::new(1, 0);
const HEADER: ChunkKey = ChunkKey::new(2, 0);
const STATUS: ChunkKey = ChunkKey::new(3, 0);
const TRACK: ChunkKey = ChunkKey::new(4, 0);
const BAR: ChunkKey = ChunkKey::new(5, 0);
const MARKER: ChunkKey = ChunkKey::new(6, 0);

/// The side of the square a key press recolours.
const MARKER_SIDE: f32 = 24.0;

/// The number of rows in the list.
const ROWS: usize = 1000;

/// The number of rows one chunk holds.
const PER_CHUNK: usize = 25;

/// The height of one row.
const ROW: f32 = 36.0;

/// The distance of the content from the window's edges.
const MARGIN: f32 = 16.0;

/// The top of the sliding bar's track.
const TRACK_TOP: f32 = 68.0;

/// The top of the port.
const TOP: f32 = 88.0;

/// The width of the sliding bar.
const BAR_WIDTH: f32 = 96.0;

/// How long one slide of the bar across its track takes.
const SLIDE_NS: u64 = 1_600_000_000;

/// A client's content, in logical pixels under a root transform that scales them to the output.
pub(crate) struct List {
    client: ClientId,
    root: OrderId,
    space: TransformId,
    node: ScrollId,
    port: ClipId,
    content: TransformId,
    bar: TransformId,
    paints: Paints,
    font: Font,
    size: [f32; 2],
    /// The offset the client last wrote as the node's base.
    base: f32,
    /// Whether the marker shows its second colour.
    marked: bool,
}

struct Paints {
    background: PaintId,
    port: PaintId,
    even: PaintId,
    odd: PaintId,
    ink: PaintId,
    track: PaintId,
    bar: PaintId,
}

impl List {
    /// Records the content of `client` for `output`, `size` logical pixels at `scale`.
    pub(crate) fn open(
        handle: &mut ClientHandle,
        client: ClientId,
        output: OutputId,
        font: Font,
        size: [f32; 2],
        scale: f32,
    ) -> Self {
        let mut tx = handle.transaction(client);
        let paints = Paints {
            background: solid(&mut tx, [0.94, 0.95, 0.97, 1.0]),
            port: solid(&mut tx, [1.0, 1.0, 1.0, 1.0]),
            even: solid(&mut tx, [0.9, 0.93, 0.99, 1.0]),
            odd: solid(&mut tx, [0.98, 0.98, 1.0, 1.0]),
            ink: solid(&mut tx, [0.1, 0.12, 0.18, 1.0]),
            track: solid(&mut tx, [0.85, 0.87, 0.92, 1.0]),
            bar: solid(&mut tx, [0.25, 0.45, 0.95, 1.0]),
        };
        let space = tx.create_transform(node(None, scaled(scale), None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let scroll = tx.create_scroll(scroll_node(space, size, 0.0));
        let port = tx.create_clip(port_clip(space, size));
        let content = tx.create_transform(node(Some(space), Matrix4::IDENTITY, Some(scroll)));
        let bar = tx.create_transform(node(Some(space), Matrix4::IDENTITY, None));

        let list = Self {
            client,
            root,
            space,
            node: scroll,
            port,
            content,
            bar,
            paints,
            font,
            size,
            base: 0.0,
            marked: false,
        };
        list.record_frame(&mut tx);
        list.record_header(&mut tx);
        list.record_status(
            &mut tx,
            "Fling the list with a touchpad, or scroll it with a wheel",
        );
        list.record_bar(&mut tx);
        list.record_marker(&mut tx);
        for chunk in 0..ROWS / PER_CHUNK {
            list.record_rows(&mut tx, chunk);
        }
        let mut entries = vec![
            OrderEntry::Chunk(BACKGROUND),
            OrderEntry::Chunk(HEADER),
            OrderEntry::Chunk(STATUS),
            OrderEntry::Chunk(TRACK),
            OrderEntry::Chunk(BAR),
            OrderEntry::Chunk(MARKER),
        ];
        entries.extend((0..ROWS / PER_CHUNK).map(|chunk| OrderEntry::Chunk(rows_key(chunk))));
        tx.splice(list.root, 0, 0, &entries);
        list.slide(&mut tx);
        handle.commit(tx);
        // Only the list coming to rest wakes the client.
        let watch = ScrollWatch {
            settle: true,
            ..ScrollWatch::default()
        };
        handle.watch_scroll(client, list.node, Some(watch));
        list
    }

    /// Lays the content out again for `size` logical pixels at `scale`.
    pub(crate) fn resize(&mut self, handle: &mut ClientHandle, size: [f32; 2], scale: f32) {
        self.size = size;
        let mut tx = handle.transaction(self.client);
        tx.set_transform(self.space, scaled(scale));
        // The base offset keeps the compositor's scrolling and any motion.
        tx.set_scroll_node(self.node, scroll_node(self.space, size, self.base));
        tx.set_clip(self.port, port_clip(self.space, size));
        self.record_frame(&mut tx);
        self.record_bar(&mut tx);
        self.record_marker(&mut tx);
        for chunk in 0..ROWS / PER_CHUNK {
            self.record_rows(&mut tx, chunk);
        }
        self.slide(&mut tx);
        handle.commit(tx);
    }

    /// Takes the client's events, and adopts the offset the list comes to rest at.
    ///
    /// On rest the client acknowledges the rest, so its base is the offset shown, and the status
    /// line shows the row at the top of the port.
    ///
    /// # Panics
    ///
    /// Panics if the compositor rejected a transaction.
    pub(crate) fn observe(&mut self, handle: &mut ClientHandle) {
        let mut events = Vec::new();
        handle.drain_events(self.client, &mut events);
        for event in events {
            match event {
                ClientEvent::Rejected { error, .. } => {
                    panic!("the example's transaction is rejected: {error}")
                }
                ClientEvent::Rerouted { routing, .. } => println!("rerouted to {routing:?}"),
                ClientEvent::Watched {
                    node,
                    cause: WatchCause::Settled,
                    offset,
                    serial,
                    ..
                } if node == self.node => {
                    let mut tx = handle.transaction(self.client);
                    tx.acknowledge_scroll(self.node, serial, offset);
                    self.base = offset[1];
                    let row = (offset[1].max(0.0) / ROW) as usize + 1;
                    println!("settled offset={} row={row}", offset[1]);
                    self.record_status(&mut tx, &format!("Resting at row {row}"));
                    handle.commit(tx);
                }
                _ => {}
            }
        }
    }

    /// Returns the routing of a gesture at `position`, in device pixels at `scale`: the list
    /// takes every gesture over its port.
    pub(crate) fn predict(&self, position: [f32; 2], scale: f32) -> ScrollRouting {
        let [x, y, width, height] = port(self.size);
        let [px, py] = [position[0] / scale, position[1] / scale];
        if (x..x + width).contains(&px) && (y..y + height).contains(&py) {
            ScrollRouting::Handled {
                node: self.node,
                client: self.client,
            }
        } else {
            ScrollRouting::Unhandled
        }
    }

    /// Returns the list's client.
    pub(crate) fn client(&self) -> ClientId {
        self.client
    }

    /// Returns the row of the first hit on a row, counting from 1.
    pub(crate) fn row(hits: &[HitResult]) -> Option<usize> {
        hits.iter().find_map(|hit| {
            let chunk = hit.chunk.owner.checked_sub(100)? as usize;
            // A row records its band and then its label.
            Some(chunk * PER_CHUNK + hit.item as usize / 2 + 1)
        })
    }

    /// Recolours the square at the header's right end, and commits it.
    pub(crate) fn toggle_marker(&mut self, handle: &mut ClientHandle) {
        self.marked = !self.marked;
        let mut tx = handle.transaction(self.client);
        self.record_marker(&mut tx);
        handle.commit(tx);
    }
}
