//! World matrices: from a transform node's local space to device space.

mod sticky;
mod thumb;

use std::collections::HashMap;

use iceblink::{Anchor, Matrix4, ScrollNode};

use crate::naive::scene::Client;

/// The world matrices of one client's transform nodes, computed on demand and kept.
pub(crate) struct Worlds<'a> {
    client: &'a Client,
    /// The matrix of each slot, with and without the scroll offsets above it.
    known: HashMap<(u32, bool), Matrix4>,
}

impl<'a> Worlds<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self {
            client,
            known: HashMap::new(),
        }
    }

    /// Returns the world matrix of the transform node at `slot`, or `None` when no node is live
    /// there.
    pub(crate) fn of(&mut self, slot: u32) -> Option<Matrix4> {
        self.matrix(slot, false)
    }

    /// Returns the matrix of `slot`, taking the scroll offset of no node when `still` is set.
    ///
    /// A fixed node evaluates its parent this way, so it holds still while any scroll above it
    /// moves.
    fn matrix(&mut self, slot: u32, still: bool) -> Option<Matrix4> {
        if let Some(known) = self.known.get(&(slot, still)) {
            return Some(*known);
        }

        let node = *self.client.transforms.at(slot)?;
        let parent = match node.parent {
            Some(parent) => {
                let fixed = matches!(node.anchor, Anchor::Fixed);
                self.matrix(parent.slot(), still || fixed)?
            }
            None => Matrix4::IDENTITY,
        };

        let anchored = match node.anchor {
            Anchor::Sticky(anchor) => {
                let scroll = self.client.scrolls.get(anchor.scroll)?;
                let [x, y] = sticky::offset(&anchor, scroll);
                Matrix4::translation(x, y, 0.0)
            }
            Anchor::Thumb(anchor) => {
                let scroll = self.client.scrolls.get(anchor.scroll)?;
                let [x, y] = thumb::offset(&anchor, scroll);
                Matrix4::translation(x, y, 0.0)
            }
            Anchor::Scrolls | Anchor::Fixed => Matrix4::IDENTITY,
        };

        let scrolled = match node.scroll {
            Some(scroll) if !still => {
                let [x, y] = current_offset(self.client.scrolls.get(scroll)?);
                Matrix4::translation(-x, -y, 0.0)
            }
            _ => Matrix4::IDENTITY,
        };

        let world = parent * anchored * node.local * scrolled;
        self.known.insert((slot, still), world);
        Some(world)
    }
}

/// Returns a scroll node's offset, clamped to its range.
pub(crate) fn current_offset(node: &ScrollNode) -> [f32; 2] {
    let largest = largest_offset(node);
    [
        node.offset[0].clamp(0.0, largest[0]),
        node.offset[1].clamp(0.0, largest[1]),
    ]
}

/// Returns a scroll node's largest offset, per axis.
pub(crate) fn largest_offset(node: &ScrollNode) -> [f32; 2] {
    [
        (node.content[0] - node.port[2]).max(0.0),
        (node.content[1] - node.port[3]).max(0.0),
    ]
}
