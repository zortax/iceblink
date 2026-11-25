//! A column of panels beside the conversation, recorded again at every width.

use iceblink::{
    ChunkKey, ClipId, ClipNode, ClipShape, Color, Matrix4, OrderEntry, OrderId, Paint, PaintId,
    PaintKind, Quad, ROUND, Transaction, TransformId,
};

use crate::items::{node, props, quad};

/// The number of panels in the column.
const PANELS: u64 = 6;

/// Where the column starts, as a share of the port's width.
const COLUMN: f32 = 0.55;

/// The gap between the column and the port's right edge, and between two panels.
const MARGIN: f32 = 12.0;

/// The height of a panel.
const HEIGHT: f32 = 130.0;

/// The radius of every panel corner.
const RADIUS: f32 = 10.0;

/// Panels in the port's content that reach from a share of the first width to the port's right
/// edge, each drawn through its own rounded clip inside the port clip.
///
/// Each step writes every panel's clip and records every panel again at the new width, and
/// moves a space that nothing is drawn in.
pub(crate) struct Panels {
    space: TransformId,
    content: TransformId,
    port: ClipId,
    panels: Vec<Panel>,
    fill: PaintId,
    ink: PaintId,
    /// A space every step moves, with nothing drawn in it.
    marker: TransformId,
}

/// One panel: its chunk, its clip and its left and top edges.
struct Panel {
    key: ChunkKey,
    clip: ClipId,
    left: f32,
    top: f32,
}

impl Panels {
    /// Creates the panels in `content` for a port `width` wide, drawn through `port`, and adds
    /// them to `order` at `at`.
    pub(crate) fn new(
        tx: &mut Transaction,
        [space, content]: [TransformId; 2],
        port: ClipId,
        (order, at): (OrderId, u32),
        width: f32,
    ) -> Self {
        let fill = tx.create_paint(Paint::Solid(Color([0.12, 0.14, 0.2, 1.0])));
        let ink = tx.create_paint(Paint::Solid(Color([0.9, 0.92, 0.96, 1.0])));
        let marker = tx.create_transform(node(Some(space), None));

        let panels = (0..PANELS)
            .map(|index| {
                let (left, top) = (
                    (width * COLUMN).round(),
                    40.0 + index as f32 * (HEIGHT + MARGIN),
                );
                let rect = [left, top, width - MARGIN - left, HEIGHT];
                Panel {
                    key: ChunkKey::new(u64::MAX - 1, index as u32),
                    clip: tx.create_clip(panel_clip(content, port, rect)),
                    left,
                    top,
                }
            })
            .collect::<Vec<_>>();
        let entries: Vec<OrderEntry> = panels
            .iter()
            .map(|panel| OrderEntry::Chunk(panel.key))
            .collect();
        tx.splice(order, at, 0, &entries);

        let panels = Self {
            space,
            content,
            port,
            panels,
            fill,
            ink,
            marker,
        };
        panels.record(tx, width, 0);
        panels
    }

    /// Writes the panels' clips and records the panels for a port `width` wide, and moves the
    /// marker space to `step` pixels right.
    pub(crate) fn record(&self, tx: &mut Transaction, width: f32, step: u32) {
        let mut marker = node(Some(self.space), None);
        marker.local = Matrix4::translation(step as f32, 0.0, 0.0);
        tx.set_transform_node(self.marker, marker);

        for panel in &self.panels {
            let rect = panel.rect(width);
            tx.set_clip(panel.clip, panel_clip(self.content, self.port, rect));
            let mut chunk = tx.record(panel.key, props(self.content, Some(panel.clip)));
            let fill = chunk.paint_ref(PaintKind::SOLID, self.fill);
            chunk.quad(Quad {
                radii: [RADIUS; 8],
                ..quad(rect, fill)
            });
            let ink = chunk.paint_ref(PaintKind::SOLID, self.ink);
            chunk.quad(quad([panel.left + 16.0, panel.top + 92.0, 24.0, 24.0], ink));
        }
    }
}

impl Panel {
    /// Returns the panel's bounds in a port `width` wide.
    fn rect(&self, width: f32) -> [f32; 4] {
        [self.left, self.top, width - MARGIN - self.left, HEIGHT]
    }
}

/// Returns the rounded clip of a panel with bounds `rect`, inside the port clip.
fn panel_clip(content: TransformId, port: ClipId, rect: [f32; 4]) -> ClipNode {
    ClipNode {
        parent: Some(port),
        space: content,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [RADIUS; 8],
            shapes: ROUND,
        },
    }
}
