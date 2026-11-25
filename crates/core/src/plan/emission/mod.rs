//! The planner's input: a frame's lowered emission stream, in paint order.

mod entry;
mod instance;

use iceblink::DeviceRect;

pub(crate) use self::entry::{Entry, NO_COVER};
pub use self::instance::{Instance, LaneKind};

/// A frame's lowered emission stream, in paint order.
///
/// The stream lists each object a frame draws with its device ink, the whole pixels the object can
/// change, cut to its clip. An instance refers to its data by an
/// [`ItemSource`](iceblink::ItemSource), and the planner reads no item data.
///
/// Paths, external quads, backdrops and groups are numbered in push order, so the `n`th of each
/// kind is index `n` of `Frame::paths`, `Frame::externals`, `Frame::backdrops` or
/// `Frame::groups`.
///
/// # Blocks
///
/// A block is a run of instances already ordered among themselves, such as a chunk recorded
/// earlier or the glyphs of one run. Each member has its order within the block,
/// [`Instance::local`], counting from one, as [`local_orders`](crate::plan::local_orders) computes
/// it. The planner places the whole block with one insertion into its bounds tree, and each member
/// takes the block's base order plus its local order less one.
///
/// Local orders stay valid while the members move together by whole device pixels. For local
/// orders that survive any device translation, compute them over the inks grown by one pixel on
/// every side.
///
/// A block holds only instances, and pushing anything else closes it.
#[derive(Clone, Debug, Default)]
pub struct Emission {
    pub(crate) entries: Vec<Entry>,
    /// The opaque cover rectangles of instances, in the order they were pushed.
    pub(crate) covers: Vec<DeviceRect>,
    /// The ink of every path, by its index in `Frame::paths`.
    pub(crate) path_inks: Vec<DeviceRect>,
    pub(crate) externals: u32,
    pub(crate) backdrops: u32,
    /// The backdrops whose source covers pixels outside the rectangle they write.
    pub(crate) wide_backdrops: u32,
    pub(crate) groups: u32,
    /// The entry index of the open block's header.
    block: Option<usize>,
    depth: u32,
}

impl Emission {
    /// Creates an empty stream.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the stream, keeping the allocations.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.covers.clear();
        self.path_inks.clear();
        self.externals = 0;
        self.backdrops = 0;
        self.wide_backdrops = 0;
        self.groups = 0;
        self.block = None;
        self.depth = 0;
    }

    /// Returns how many entries the stream holds, block headers included.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Reports whether nothing was pushed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Appends an instance.
    pub fn push(&mut self, instance: Instance) {
        self.push_instance(instance, NO_COVER);
    }

    /// Appends a quad that writes an opaque colour to every pixel of `cover`.
    ///
    /// The quad must meet these conditions:
    ///
    /// - Its fill is an opaque solid colour, and it has no border or a solid border in an opaque
    ///   solid colour.
    /// - Its world matrix is a translation or a positive scale.
    /// - Every link of its clip chain is a rectangle without radii under such a matrix.
    /// - `cover` is its device bounds cut to the clip, inset by its largest radius, then rounded
    ///   inwards and inset by one pixel with [`inner_pixels`](crate::plan::inner_pixels), and it
    ///   lies inside the output.
    ///
    /// The planner keeps a cover only for a quad drawn before every group.
    pub fn push_cover(&mut self, instance: Instance, cover: DeviceRect) {
        debug_assert_eq!(instance.lane, LaneKind::Quads, "only a quad is a cover");
        let index = self.covers.len() as u32;
        self.covers.push(cover);
        self.push_instance(instance, index);
    }

    /// Starts a block, which holds the instances pushed until [`Emission::end_block`] or until
    /// anything other than an instance is pushed.
    ///
    /// A block already open is closed.
    pub fn begin_block(&mut self) {
        self.block = Some(self.entries.len());
        self.entries.push(Entry::Block { len: 0 });
    }

    /// Closes the open block, if any.
    pub fn end_block(&mut self) {
        self.block = None;
    }

    /// Appends the path at the next index of `Frame::paths`, drawn through the frame clip slot
    /// `clip`, and returns that index.
    ///
    /// `ink` holds the pixels the path changes, and `reach` the pixels its outline covers before
    /// any clip.
    pub fn push_path(&mut self, ink: DeviceRect, reach: DeviceRect, clip: u32) -> u32 {
        self.block = None;
        let index = self.path_inks.len() as u32;
        self.path_inks.push(ink);
        self.entries.push(Entry::Path {
            index,
            ink,
            reach,
            clip,
        });
        index
    }

    /// Appends the external quad at the next index of `Frame::externals` and returns that index.
    pub fn push_external(&mut self, ink: DeviceRect) -> u32 {
        self.block = None;
        let index = self.externals;
        self.externals += 1;
        self.entries.push(Entry::External { index, ink });
        index
    }

    /// Appends the backdrop at the next index of `Frame::backdrops` and returns that index.
    ///
    /// `ink` is the rectangle the backdrop writes, and `reads_outside` says whether its source
    /// covers pixels outside `ink`.
    pub fn push_backdrop(&mut self, ink: DeviceRect, reads_outside: bool) -> u32 {
        self.block = None;
        let index = self.backdrops;
        self.backdrops += 1;
        self.wide_backdrops += u32::from(reads_outside);
        self.entries.push(Entry::Backdrop { index, ink });
        index
    }

    /// Starts the group at the next index of `Frame::groups`, which writes `bounds`, and returns
    /// that index.
    ///
    /// Groups nest, and every group started is ended with [`Emission::end_group`].
    pub fn begin_group(&mut self, bounds: DeviceRect) -> u32 {
        self.begin_reading_group(bounds, 0)
    }

    /// Starts a group as [`Emission::begin_group`] does, for filters that read `reach` device
    /// pixels past what they write.
    ///
    /// Content inside the group is drawn where it meets the draw damage grown by the reach of
    /// every enclosing group, past the output too.
    pub fn begin_reading_group(&mut self, bounds: DeviceRect, reach: i32) -> u32 {
        self.block = None;
        let index = self.groups;
        self.groups += 1;
        self.depth += 1;
        self.entries.push(Entry::GroupBegin {
            index,
            bounds,
            reach,
        });
        index
    }

    /// Ends the innermost open group.
    pub fn end_group(&mut self) {
        debug_assert!(self.depth > 0, "a group ends that never began");
        self.block = None;
        self.depth = self.depth.saturating_sub(1);
        self.entries.push(Entry::GroupEnd);
    }

    /// Returns the ink of every path, by index in `Frame::paths`.
    pub fn path_inks(&self) -> &[DeviceRect] {
        &self.path_inks
    }

    /// Appends an instance with the cover index `cover`.
    fn push_instance(&mut self, instance: Instance, cover: u32) {
        if let Some(header) = self.block
            && let Some(Entry::Block { len }) = self.entries.get_mut(header)
        {
            *len += 1;
        }
        self.entries.push(Entry::Instance { instance, cover });
    }
}
