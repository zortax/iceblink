//! The offset that holds a sticky node inside its scroll port.

use iceblink::{ScrollNode, StickyAnchor};

use crate::naive::space::current_offset;

/// Returns the sticky translation of `anchor` under the current offset of `scroll`.
pub(super) fn offset(anchor: &StickyAnchor, scroll: &ScrollNode) -> [f32; 2] {
    let [scrolled_x, scrolled_y] = current_offset(scroll);
    let port = scroll.port;
    let [top, right, bottom, left] = anchor.inset;

    let x = axis(Axis {
        port: (port[0], port[2]),
        rect: (anchor.rect[0] - scrolled_x, anchor.rect[2]),
        limit: (anchor.limit[0] - scrolled_x, anchor.limit[2]),
        start: left,
        end: right,
    });
    let y = axis(Axis {
        port: (port[1], port[3]),
        rect: (anchor.rect[1] - scrolled_y, anchor.rect[3]),
        limit: (anchor.limit[1] - scrolled_y, anchor.limit[3]),
        start: top,
        end: bottom,
    });
    [x, y]
}

/// One axis of the sticky rule, each span as its start and length.
struct Axis {
    port: (f32, f32),
    rect: (f32, f32),
    limit: (f32, f32),
    /// The inset from the port's start, NaN when that edge is free.
    start: f32,
    /// The inset from the port's end, NaN when that edge is free.
    end: f32,
}

fn axis(axis: Axis) -> f32 {
    let (port, port_len) = axis.port;
    let (rect, rect_len) = axis.rect;
    let (limit, limit_len) = axis.limit;

    let mut sigma = 0.0_f32;
    if !axis.start.is_nan() {
        sigma = sigma.max(port + axis.start - rect);
        sigma = sigma.min(limit + limit_len - rect - rect_len);
    }

    if !axis.end.is_nan() {
        sigma = sigma.min(port + port_len - axis.end - rect - rect_len);
        sigma = sigma.max(limit - rect);
    }

    sigma
}
