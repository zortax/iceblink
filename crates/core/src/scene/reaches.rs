//! The users of a clip node with the reach of each.

use iceblink::{ChunkKey, DeviceRect};
use rustc_hash::FxHashMap;

use crate::geometry::bounds::DeviceBox;

/// The chunks that refer to a clip node, each with what its inks were last stored with.
///
/// Users come out in no particular order.
#[derive(Debug, Default)]
pub(crate) struct Reaches {
    users: Vec<Reach>,
    positions: FxHashMap<ChunkKey, u32>,
}

/// One user of a clip node.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reach {
    pub(crate) key: ChunkKey,
    /// The device box that holds the chunk's items before clips cut them, if it has one.
    pub(crate) reach: Option<DeviceBox>,
    /// Whether the chunk is drawn on the device, no scroll node can carry it, it reads nothing
    /// around itself and it has no local clips.
    pub(crate) plain: bool,
    /// The chunk as a clip write restills it, when it is carried, reads nothing around itself,
    /// refers to no other clip node, and its carry follows the still pixels of a clip node.
    pub(crate) carried: Option<CarriedReach>,
}

/// A carried user of a clip node, as a write of the clip restills it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CarriedReach {
    /// The union of the chunk's item inks in the scroll node's content space.
    pub(crate) content: Option<DeviceRect>,
    /// The slot of the scroll node that carries the chunk.
    pub(crate) scroll: u32,
    /// The slot of the clip node whose still pixels the chunk is cut to.
    pub(crate) root: u32,
}

impl Reaches {
    /// Adds chunk `key`, which is neither plain nor carried until its inks are stored, unless it
    /// is a user already.
    pub(crate) fn insert(&mut self, key: ChunkKey) {
        if self.positions.contains_key(&key) {
            return;
        }

        self.positions.insert(key, self.users.len() as u32);
        self.users.push(Reach {
            key,
            reach: None,
            plain: false,
            carried: None,
        });
    }

    /// Stores `user`, adding it if it is not a user.
    pub(crate) fn set(&mut self, user: Reach) {
        let key = user.key;
        match self.positions.get(&key) {
            Some(&position) => self.users[position as usize] = user,
            None => {
                self.positions.insert(key, self.users.len() as u32);
                self.users.push(user);
            }
        }
    }

    /// Removes user `key`, if it is one.
    pub(crate) fn remove(&mut self, key: ChunkKey) {
        let Some(position) = self.positions.remove(&key) else {
            return;
        };

        self.users.swap_remove(position as usize);
        if let Some(moved) = self.users.get(position as usize) {
            self.positions.insert(moved.key, position);
        }
    }

    pub(crate) fn users(&self) -> &[Reach] {
        &self.users
    }
}
