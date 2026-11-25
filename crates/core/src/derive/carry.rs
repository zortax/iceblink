//! The inks of chunks whose content one scroll node moves, in that node's content space.

use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, SpaceRef};

use crate::geometry::bounds::{DeviceBox, INK_LIMIT};
use crate::geometry::coverage::Partial;
use crate::geometry::ink::local_ink;
use crate::geometry::split::{item_split, local_splits};
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::follows::{ClipSplit, Follows, StillRoot};
use crate::scene::inks::{Carry, Still, moved};
use crate::scene::kind::Kind;

/// The rectangle content-space inks are cut to.
// Far larger than `INK_LIMIT`, so an ink cut here, moved by a travel of under 2^27 pixels and cut
// to `INK_LIMIT`, equals the ink measured after the move.
const CONTENT_LIMIT: DeviceRect = Rect::new(
    Point::new(-(1 << 28), -(1 << 28)),
    Size::new(1 << 29, 1 << 29),
);

/// Computes the item inks of `record` into `items`, in the content space of the scroll node that
/// carries it, and returns the carry, or `None` when no scroll node carries it.
///
/// A scroll node carries a chunk when all of these hold:
///
/// 1. Every space the chunk refers to follows that node's offset alone.
/// 2. Every clip link an item with an ink is drawn through follows that offset or none.
/// 3. The links that follow none cut every item with an ink to the same box of whole pixels.
/// 4. The chunk holds no backdrop that reads outside what it writes.
///
/// The inks are cut by the links that follow the offset. Cut by the box as well and moved by the
/// node's travel, they equal the inks `chunk_inks` computes.
pub(crate) fn carried(
    scene: &Scene,
    record: &ChunkRecord,
    items: &mut Vec<Option<DeviceRect>>,
) -> Option<Carry> {
    if record.derived.reads_around {
        return None;
    }

    let scroll = carrier(scene, record)?;
    let chunk = &record.recorded;
    let locals = local_splits(scene, chunk, record.offset);
    let back = scene.travel(scroll).map(|travel| -travel);

    let mut still = Stills::default();
    items.clear();
    for &op in &chunk.ops {
        let Some(local) = local_ink(chunk, op, &record.derived.runs) else {
            items.push(None);
            continue;
        };

        let slot = if local.space == SpaceRef::INHERIT {
            chunk.props.space.slot()
        } else {
            local.space.0
        };
        let world = scene.transforms.at(slot)?.world;
        let split = item_split(scene, chunk, &locals, local.clip);
        if !matches!(split.follows, Follows::Still) && split.follows != Follows::Scroll(scroll) {
            return None;
        }
        still.add(&split)?;

        let ink = local
            .on_device(&world, record.offset)
            .intersect(split.moving)
            .pixels(CONTENT_LIMIT)
            .map(|ink| moved(ink, back));
        items.push(ink);
    }

    Some(Carry {
        scroll,
        still: still.finish(scene)?,
    })
}

/// The still bounds of a chunk's items, taken in one clip split at a time.
pub(crate) struct Stills {
    bounds: Option<DeviceBox>,
    partial: Option<Partial>,
    roots: Roots,
}

/// The still roots of the splits taken in.
#[derive(Clone, Copy, PartialEq)]
enum Roots {
    None,
    One(StillRoot),
    Several,
}

impl Default for Stills {
    fn default() -> Self {
        Self {
            bounds: None,
            partial: Some(Partial::NONE),
            roots: Roots::None,
        }
    }
}

impl Stills {
    /// Takes in the split of an item's clip chain, or returns `None` when its still bounds
    /// differ from those taken in before.
    pub(crate) fn add(&mut self, split: &ClipSplit) -> Option<()> {
        match self.bounds {
            Some(held) if held != split.still => return None,
            _ => self.bounds = Some(split.still),
        }

        self.partial = self
            .partial
            .zip(split.partial)
            .map(|(held, added)| held.and(added));
        self.roots = match self.roots {
            Roots::None => Roots::One(split.root),
            Roots::One(held) if held == split.root => Roots::One(held),
            Roots::One(_) | Roots::Several => Roots::Several,
        };
        Some(())
    }

    /// Returns the still pixels the splits taken in cut to, or `None` when their edges are not
    /// whole.
    ///
    /// The result refers to a clip node when every split's still root is that node and the node
    /// holds the same still pixels.
    pub(crate) fn finish(self, scene: &Scene) -> Option<Still> {
        let pixels = whole_pixels(self.bounds.unwrap_or(DeviceBox::EVERYWHERE))?;

        if let Roots::One(StillRoot::Node(slot)) = self.roots
            && scene
                .clips
                .at(slot)
                .is_some_and(|record| record.still == Some(pixels))
        {
            return Some(Still::Node(slot));
        }
        Some(Still::Fixed {
            pixels,
            partial: self.partial,
        })
    }
}

/// Returns the scroll node every space of a chunk follows alone, if there is one.
pub(crate) fn carrier(scene: &Scene, record: &ChunkRecord) -> Option<u32> {
    let mut scroll = None;
    for object in &record.derived.deps {
        if object.kind != Kind::Transform {
            continue;
        }
        let Follows::Scroll(follows) = scene.transforms.at(object.slot)?.follows else {
            return None;
        };
        if scroll.is_some_and(|held| held != follows) {
            return None;
        }
        scroll = Some(follows);
    }
    scroll
}

/// Returns the pixels of a box whose finite edges are whole, cut to `INK_LIMIT`.
pub(crate) fn whole_pixels(still: DeviceBox) -> Option<DeviceRect> {
    let edges = [still.x0, still.y0, still.x1, still.y1];
    if edges.iter().any(|edge| *edge != edge.floor()) {
        return None;
    }
    still.pixels(INK_LIMIT)
}
