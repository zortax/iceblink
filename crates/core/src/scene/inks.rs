//! Stored inks of chunks and order nodes, on the device or in a scroll node's content space.
//!
//! The ink of content that only one scroll node moves is kept in that node's content space:
//! device pixels less the node's travel, the whole-pixel distance its content has moved since
//! the node was created. Such content is carried by the node. A whole-pixel scroll then moves
//! the content by adding to the travel, without touching the stored inks.
//!
//! The still pixels carried content is cut to mostly come from one clip node's chain, such as a
//! scroll port's, and the carry then refers to that node. A write that moves the port's edges then
//! changes no stored ink either. Every reader turns a stored ink into device pixels with the
//! current travel and still pixels.

use iceblink::DeviceRect;
use iceblink::geom::{Point, Rect, Size};

use crate::geometry::bounds::DeviceBox;
use crate::geometry::coverage::Partial;

/// The empty rectangle at the origin.
pub(crate) const NO_PIXELS: DeviceRect = Rect::new(Point::new(0, 0), Size::new(0, 0));

/// The scroll node whose content space a stored ink lies in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Carry {
    /// The scroll node's slot.
    pub(crate) scroll: u32,
    /// The device pixels the content is cut to, which do not move with the scroll node.
    pub(crate) still: Still,
}

/// The still pixels carried content is cut to, and where the clip links that cut it cover in
/// part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Still {
    /// The still pixels and partial cover of the chain of the clip node at this slot, which its
    /// record holds, so they follow every write of the chain.
    Node(u32),
    /// Pixels of their own. The partial cover is `None` when it cannot be bounded.
    Fixed {
        pixels: DeviceRect,
        partial: Option<Partial>,
    },
}

/// The ink of every item of a chunk, and their union.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ChunkInks {
    /// The ink of each item by its position in the chunk, or `None` for an item that changes no
    /// pixel.
    items: Vec<Option<DeviceRect>>,
    union: Option<DeviceRect>,
    /// The device box that holds every item before clips cut it, or `None` when no item has a
    /// box. It is `None` for carried inks.
    reach: Option<DeviceBox>,
    /// The scroll node that carries the chunk, whose content space the inks lie in before they
    /// are cut to its still pixels.
    carry: Option<Carry>,
}

impl ChunkInks {
    /// Returns the inks of a chunk no scroll node carries, from device inks and the device box
    /// that holds every item before clips cut it.
    pub(crate) fn on_device(items: Vec<Option<DeviceRect>>, reach: Option<DeviceBox>) -> Self {
        Self {
            union: union_of(&items),
            items,
            reach,
            carry: None,
        }
    }

    /// Returns the inks of a chunk that `carry` carries, from inks in its content space.
    pub(crate) fn carried(items: Vec<Option<DeviceRect>>, carry: Carry) -> Self {
        Self {
            union: union_of(&items),
            items,
            reach: None,
            carry: Some(carry),
        }
    }

    /// Removes every ink, keeping the capacity of the item list.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.union = None;
        self.reach = None;
        self.carry = None;
    }

    /// Returns a copy of the inks whose item list is `items`, which must be empty.
    pub(crate) fn copy_into(&self, mut items: Vec<Option<DeviceRect>>) -> Self {
        items.extend_from_slice(&self.items);
        Self {
            items,
            union: self.union,
            reach: self.reach,
            carry: self.carry,
        }
    }

    /// Takes the item list out, empty, for the next inks to fill.
    pub(crate) fn take_items(&mut self) -> Vec<Option<DeviceRect>> {
        let mut items = core::mem::take(&mut self.items);
        items.clear();
        items
    }

    /// Returns the scroll node that carries the chunk, if one does.
    pub(crate) fn carry(&self) -> Option<Carry> {
        self.carry
    }

    /// Replaces the carry of carried inks, keeping the inks in the scroll node's content space.
    pub(crate) fn set_carry(&mut self, carry: Carry) {
        if self.carry.is_some() {
            self.carry = Some(carry);
        }
    }

    /// Returns the device box that holds every item before clips cut it, for inks on the device.
    pub(crate) fn reach(&self) -> Option<DeviceBox> {
        self.reach
    }

    /// Returns the union of the item inks as stored, before any cut to still pixels.
    pub(crate) fn content(&self) -> Option<DeviceRect> {
        self.union
    }

    /// Returns the inks on the device, with the carrying scroll node's content moved by
    /// `travel` and cut to `still`.
    pub(crate) fn at(&self, travel: [i32; 2], still: DeviceRect) -> DeviceInks<'_> {
        DeviceInks {
            inks: self,
            travel,
            still,
        }
    }

    /// Takes the stored inks out, leaving none.
    pub(crate) fn take(&mut self) -> Self {
        core::mem::take(self)
    }
}

/// A chunk's inks on the device.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DeviceInks<'a> {
    inks: &'a ChunkInks,
    travel: [i32; 2],
    /// The still pixels carried inks are cut to.
    still: DeviceRect,
}

impl DeviceInks<'_> {
    /// Returns how many items the chunk holds.
    pub(crate) fn len(&self) -> usize {
        self.inks.items.len()
    }

    /// Returns the ink of the item at `position`.
    pub(crate) fn item(&self, position: usize) -> Option<DeviceRect> {
        let ink = self.inks.items.get(position).copied().flatten()?;
        match self.inks.carry {
            Some(_) => self.still.intersection(moved(ink, self.travel)),
            None => Some(ink),
        }
    }

    /// Returns the ink of every item, by position.
    pub(crate) fn items(&self) -> impl Iterator<Item = Option<DeviceRect>> + '_ {
        (0..self.len()).map(|position| self.item(position))
    }

    /// Returns the union of the item inks.
    pub(crate) fn union(&self) -> Option<DeviceRect> {
        if self.inks.carry.is_none() {
            return self.inks.union;
        }

        let content = moved(self.inks.union?, self.travel);
        if self.still.contains_rect(content) {
            return Some(content);
        }
        if !self.still.intersects(content) {
            return None;
        }

        self.items().flatten().reduce(|union, ink| union.union(ink))
    }
}

/// The stored ink of an order node's subtree, which bounds the union of its items' inks.
///
/// A carried node stores its content's union, which is moved by the travel and cut to the still
/// pixels. That is wider than the union of its entries' cut inks when an entry lies partly
/// outside the still pixels. A node above a carried node can be wider in the same way.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NodeInk {
    ink: Option<DeviceRect>,
    carry: Option<Carry>,
    /// Whether the ink can be wider than the union of the subtree's item inks.
    wide: bool,
}

impl NodeInk {
    /// Returns the ink of a node no scroll node carries, from its device ink, which is `wide`
    /// when it can be wider than the union of the subtree's item inks.
    pub(crate) fn on_device(ink: Option<DeviceRect>, wide: bool) -> Self {
        Self {
            ink,
            carry: None,
            wide,
        }
    }

    /// Returns the ink of a node that `carry` carries, from its content's union in the scroll
    /// node's content space.
    pub(crate) fn carried(content: Option<DeviceRect>, carry: Carry) -> Self {
        Self {
            ink: content,
            carry: Some(carry),
            wide: true,
        }
    }

    /// Returns the same ink, marked as possibly wider than the union of the subtree's item inks.
    pub(crate) fn widened(self) -> Self {
        Self { wide: true, ..self }
    }

    /// Returns the scroll node that carries the node, if one does.
    pub(crate) fn carry(&self) -> Option<Carry> {
        self.carry
    }

    /// Returns the ink as stored, before any cut to still pixels.
    pub(crate) fn content(&self) -> Option<DeviceRect> {
        self.ink
    }

    /// Reports whether the ink can be wider than the union of the subtree's item inks.
    pub(crate) fn wide(&self) -> bool {
        self.wide
    }

    /// Returns the ink on the device, with the carrying scroll node's content moved by
    /// `travel` and cut to `still`.
    pub(crate) fn at(&self, travel: [i32; 2], still: DeviceRect) -> Option<DeviceRect> {
        let ink = self.ink?;
        match self.carry {
            Some(_) => still.intersection(moved(ink, travel)),
            None => Some(ink),
        }
    }
}

/// Returns the union of the inks of `items`, or `None` when no item has one.
fn union_of(items: &[Option<DeviceRect>]) -> Option<DeviceRect> {
    items
        .iter()
        .flatten()
        .copied()
        .reduce(|union, ink| union.union(ink))
}

/// Returns `rect` moved by `by` whole pixels.
pub(crate) fn moved(rect: DeviceRect, by: [i32; 2]) -> DeviceRect {
    let origin = Point::new(
        rect.origin.x.saturating_add(by[0]),
        rect.origin.y.saturating_add(by[1]),
    );
    DeviceRect::new(origin, rect.size)
}
