//! Packing a frame's passes into scratch layers, and the layers into rounds.

use iceblink::DeviceRect;
use iceblink::geom::{Device, Point, Rect, Size};

use crate::vector::resource::VectorTarget;

/// How much scratch one round may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity {
    /// The side of one layer, in device pixels.
    ///
    /// A layer grows past it to hold a pass wider or taller than this, so every pass fits.
    pub side: i32,
    /// The most layers one round may use.
    pub layers: u32,
}

impl Capacity {
    /// Eight layers of 2048 by 2048 pixels.
    // 128 MiB of straight colour at most, room for about 33 million pixels of passes per round.
    pub const DEFAULT: Self = Self {
        side: 2048,
        layers: 8,
    };
}

impl Default for Capacity {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Where one pass is rasterised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The round that rasterises the pass.
    pub round: usize,
    /// The layer the pass is rasterised into.
    pub target: VectorTarget,
    /// The pass's bin in that layer, of the pass region's size.
    pub bin: DeviceRect,
}

/// A frame's passes, each placed in a bin of a scratch layer, and the layers grouped into rounds.
///
/// Bins in one layer of one round never share a texel. Rounds follow the order of the passes,
/// and a round uses at most `Capacity::layers` layers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layering {
    /// One placement per pass, in pass order.
    placements: Vec<Placement>,
    /// The width every layer needs.
    width: u32,
    /// The height every layer needs.
    height: u32,
    /// The most layers any round uses.
    layers: u32,
    /// How many rounds the passes take.
    rounds: usize,
}

impl Layering {
    /// Packs the pass `regions`, in order, into rounds no larger than `capacity`.
    ///
    /// A bin goes to the right of the last one, on a new shelf below when the shelf is full, on a
    /// new layer when the layer is full, and into a new round when the round is full. The shelf
    /// width follows the total area, so a few small passes use a small square of one layer. An
    /// empty region takes an empty bin.
    pub fn of(regions: &[DeviceRect], capacity: Capacity) -> Self {
        let mut shelves = Shelves::new(regions, capacity);
        let placements = regions
            .iter()
            .map(|region| shelves.place(region.size.non_negative()))
            .collect();
        Self {
            placements,
            width: shelves.width as u32,
            height: shelves.height as u32,
            layers: shelves.layers,
            rounds: if regions.is_empty() {
                0
            } else {
                shelves.round + 1
            },
        }
    }

    /// Returns where pass `index` goes, or `None` past the last pass.
    pub fn placement(&self, index: usize) -> Option<Placement> {
        self.placements.get(index).copied()
    }

    /// Returns the layer of pass `index`, or `VectorTarget::NONE` past the last pass.
    pub fn target(&self, index: usize) -> VectorTarget {
        self.placement(index)
            .map_or(VectorTarget::NONE, |placement| placement.target)
    }

    /// Returns the most layers any round uses.
    pub fn layers(&self) -> u32 {
        self.layers
    }

    /// Returns how many rounds the passes take.
    pub fn rounds(&self) -> usize {
        self.rounds
    }

    /// Returns the width and height every layer needs, at least one texel each.
    pub fn extent(&self) -> (u32, u32) {
        (self.width.max(1), self.height.max(1))
    }
}

/// The packing cursor.
struct Shelves {
    /// The widest a shelf may be.
    shelf: i32,
    /// The tallest a layer may be.
    page: i32,
    /// The most layers a round may use.
    ceiling: u32,
    /// The round being filled.
    round: usize,
    /// The layer being filled.
    layer: u32,
    /// Where the next bin starts on the open shelf.
    x: i32,
    /// The top of the open shelf.
    y: i32,
    /// The height of the open shelf.
    row: i32,
    /// The widest any layer reaches.
    width: i32,
    /// The tallest any layer reaches.
    height: i32,
    /// The most layers any round opens.
    layers: u32,
}

impl Shelves {
    /// Returns a cursor at the start of the first layer, sized for `regions`.
    fn new(regions: &[DeviceRect], capacity: Capacity) -> Self {
        let sizes = regions.iter().map(|region| region.size.non_negative());
        let widest = sizes.clone().map(|size| size.width).max().unwrap_or(0);
        let tallest = sizes.clone().map(|size| size.height).max().unwrap_or(0);
        let area: i64 = sizes
            .map(|size| i64::from(size.width) * i64::from(size.height))
            .sum();
        let side = capacity.side.max(1);
        let square = (area as f64).sqrt().ceil() as i32;
        Self {
            shelf: square.clamp(widest.max(1), side.max(widest)),
            page: side.max(tallest),
            ceiling: capacity.layers.max(1),
            round: 0,
            layer: 0,
            x: 0,
            y: 0,
            row: 0,
            width: 0,
            height: 0,
            layers: 0,
        }
    }

    /// Places one bin of `size`.
    fn place(&mut self, size: Size<i32, Device>) -> Placement {
        if size.width == 0 || size.height == 0 {
            return self.at(Point::new(0, 0), Size::new(0, 0));
        }

        if self.x > 0 && self.x + size.width > self.shelf {
            self.y += self.row;
            self.x = 0;
            self.row = 0;
        }
        if self.y > 0 && self.y + size.height > self.page {
            self.next_layer();
        }

        let origin = Point::new(self.x, self.y);
        self.x += size.width;
        self.row = self.row.max(size.height);
        self.width = self.width.max(self.x);
        self.height = self.height.max(self.y + self.row);
        self.layers = self.layers.max(self.layer + 1);
        self.at(origin, size)
    }

    /// Moves to the start of the next layer, or of the next round when this one is full.
    fn next_layer(&mut self) {
        self.layer += 1;
        if self.layer >= self.ceiling {
            self.layer = 0;
            self.round += 1;
        }
        self.x = 0;
        self.y = 0;
        self.row = 0;
    }

    /// Returns a placement in the open layer.
    fn at(&self, origin: Point<i32, Device>, size: Size<i32, Device>) -> Placement {
        Placement {
            round: self.round,
            target: VectorTarget(u64::from(self.layer)),
            bin: Rect::new(origin, size),
        }
    }
}
