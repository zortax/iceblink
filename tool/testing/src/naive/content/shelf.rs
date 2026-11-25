//! Shelf packing of tiles onto a page.

/// The free space of a page, as rows of tiles.
///
/// Every tile is kept `gap` texels away from the tiles after it on its row and below it.
#[derive(Clone, Debug)]
pub(crate) struct Shelves {
    size: [u32; 2],
    gap: u32,
    shelves: Vec<Shelf>,
}

#[derive(Clone, Copy, Debug)]
struct Shelf {
    y: u32,
    height: u32,
    /// Where the next tile on the shelf starts.
    x: u32,
}

impl Shelves {
    pub(crate) fn new(size: [u32; 2], gap: u32) -> Self {
        Self {
            size,
            gap,
            shelves: Vec::new(),
        }
    }

    /// Places a tile of `size` texels and returns its corner, or `None` when the page has no room.
    pub(crate) fn place(&mut self, size: [u32; 2]) -> Option<[u32; 2]> {
        if size[0] > self.size[0] || size[1] > self.size[1] {
            return None;
        }

        // The gap may reach past the page's far edges.
        let [width, height] = [size[0] + self.gap, size[1] + self.gap];

        for shelf in &mut self.shelves {
            if shelf.height >= height && shelf.x + size[0] <= self.size[0] {
                let at = [shelf.x, shelf.y];
                shelf.x += width;
                return Some(at);
            }
        }

        let y = self.shelves.last().map_or(0, |last| last.y + last.height);
        if y + size[1] > self.size[1] {
            return None;
        }

        self.shelves.push(Shelf {
            y,
            height,
            x: width,
        });
        Some([0, y])
    }
}
