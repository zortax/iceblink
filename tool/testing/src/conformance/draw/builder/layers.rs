//! The groups and backdrops of a frame.

use iceblink::{FrameBackdrop, FrameGroup};

use crate::conformance::draw::builder::FrameBuilder;
use crate::conformance::draw::builder::batches::Entry;
use crate::conformance::draw::builder::ink::{cut, rounded};

impl FrameBuilder {
    /// Opens a group, with its bounds cut to the output. Items until the matching
    /// [`FrameBuilder::end_group`] draw into it.
    pub fn begin_group(&mut self, mut group: FrameGroup) {
        let index = self.frame.groups.len() as u32;
        group.bounds = cut(group.bounds, self.output);
        let ink = rounded(group.bounds, self.output);
        self.frame.groups.push(group);
        self.open.push(index);
        self.entries.push((Entry::GroupBegin(index), ink));
    }

    /// Closes the innermost open group.
    pub fn end_group(&mut self) {
        let index = self.open.pop().expect("a group is open");
        let ink = rounded(self.frame.groups[index as usize].bounds, self.output);
        self.entries.push((Entry::GroupEnd(index), ink));
    }

    /// Adds a backdrop.
    pub fn backdrop(&mut self, backdrop: FrameBackdrop) {
        let index = self.frame.backdrops.len() as u32;
        let ink = rounded(backdrop.bounds, self.output);
        self.frame.backdrops.push(backdrop);
        self.entries.push((Entry::Backdrop(index), ink));
    }
}
