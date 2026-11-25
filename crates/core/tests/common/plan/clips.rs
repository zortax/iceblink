//! A frame clip table of parent-linked chains.

use std::collections::HashMap;

use iceblink::FRAME_NO_CLIP;
use iceblink_core::plan::ClipChains;

/// One link of a chain: an opaque name, and whether it is a mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Link {
    pub(crate) name: u32,
    pub(crate) mask: bool,
}

/// A frame clip table that interns chains by parent and link, so equal chains share a slot.
#[derive(Debug, Default)]
pub(crate) struct ClipTable {
    nodes: Vec<(u32, Link)>,
    interned: HashMap<(u32, Link), u32>,
    names: u32,
}

impl ClipTable {
    /// Returns a chain of one new rectangle link.
    pub(crate) fn only(&mut self) -> u32 {
        self.push(FRAME_NO_CLIP)
    }

    /// Returns `parent` extended by a new rectangle link.
    pub(crate) fn push(&mut self, parent: u32) -> u32 {
        self.names += 1;
        self.intern(
            parent,
            Link {
                name: self.names,
                mask: false,
            },
        )
    }

    /// Returns `parent` extended by a new mask link.
    pub(crate) fn mask(&mut self, parent: u32) -> u32 {
        self.names += 1;
        self.intern(
            parent,
            Link {
                name: self.names,
                mask: true,
            },
        )
    }

    /// Returns the links of `clip`, outermost first.
    pub(crate) fn links(&self, clip: u32) -> Vec<Link> {
        let mut links = Vec::new();
        let mut cursor = clip;
        while cursor != FRAME_NO_CLIP {
            let (parent, link) = self.nodes[cursor as usize];
            links.push(link);
            cursor = parent;
        }
        links.reverse();
        links
    }

    fn intern(&mut self, parent: u32, link: Link) -> u32 {
        if let Some(slot) = self.interned.get(&(parent, link)) {
            return *slot;
        }

        let slot = self.nodes.len() as u32;
        self.nodes.push((parent, link));
        self.interned.insert((parent, link), slot);
        slot
    }
}

impl ClipChains for ClipTable {
    fn parent(&self, clip: u32) -> u32 {
        self.nodes[clip as usize].0
    }

    fn has_mask(&self, clip: u32) -> bool {
        self.links(clip).iter().any(|link| link.mask)
    }

    fn residual(&mut self, clip: u32, ancestor: u32) -> u32 {
        let mut below = Vec::new();
        let mut cursor = clip;
        while cursor != ancestor && cursor != FRAME_NO_CLIP {
            let (parent, link) = self.nodes[cursor as usize];
            below.push(link);
            cursor = parent;
        }
        below
            .into_iter()
            .rev()
            .fold(FRAME_NO_CLIP, |chain, link| self.intern(chain, link))
    }
}
