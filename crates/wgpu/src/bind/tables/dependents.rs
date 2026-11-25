//! Which clips depend on which spaces, so a moved space resolves only those clips again.

use crate::bind::tables::clips::Membership;

/// The clips that depend on each space.
// An entry whose clip no longer holds the membership stays in its list until the space is visited.
// The lists are rebuilt from the memberships once stale entries outnumber live ones.
#[derive(Clone, Debug, Default)]
pub(crate) struct Dependents {
    /// Per space, the clips that moved links in it onto the device.
    users: Vec<Vec<u32>>,
    /// Per space, the clips that keep it.
    keepers: Vec<Vec<u32>>,
    /// Per clip, its memberships.
    registered: Vec<Vec<Membership>>,
    /// How many entries the lists hold.
    edges: usize,
    /// How many entries of the lists are stale.
    stale: usize,
}

impl Dependents {
    /// Records the memberships of `clip`, replacing the ones it held.
    pub(crate) fn register(&mut self, clip: u32, memberships: &[Membership]) {
        let at = clip as usize;
        if self.registered.len() <= at {
            self.registered.resize_with(at + 1, Vec::new);
        }
        let held = std::mem::take(&mut self.registered[at]);
        self.stale += held.iter().filter(|m| !memberships.contains(m)).count();
        for membership in memberships {
            if !held.contains(membership) {
                self.add(clip, *membership);
            }
        }
        self.registered[at] = memberships.to_vec();
        if self.stale > (self.edges - self.stale) + 1024 {
            self.rebuild();
        }
    }

    /// Calls `found` with every clip that uses `space`, or keeps it when `keepers` is set.
    pub(crate) fn members(&mut self, space: u32, keepers: bool, mut found: impl FnMut(u32)) {
        let (lists, membership) = if keepers {
            (&mut self.keepers, Membership::Keeps(space))
        } else {
            (&mut self.users, Membership::Uses(space))
        };
        let Some(list) = lists.get_mut(space as usize) else {
            return;
        };
        let registered = &self.registered;
        let before = list.len();
        list.retain(|&clip| {
            let live = registered
                .get(clip as usize)
                .is_some_and(|held| held.contains(&membership));
            if live {
                found(clip);
            }
            live
        });
        let dropped = before - list.len();
        self.stale = self.stale.saturating_sub(dropped);
        self.edges -= dropped;
    }

    /// Forgets every membership.
    pub(crate) fn clear(&mut self) {
        self.users.clear();
        self.keepers.clear();
        self.registered.clear();
        self.edges = 0;
        self.stale = 0;
    }

    /// Rebuilds the lists from the memberships.
    fn rebuild(&mut self) {
        self.users.iter_mut().for_each(Vec::clear);
        self.keepers.iter_mut().for_each(Vec::clear);
        self.edges = 0;
        self.stale = 0;
        let registered = std::mem::take(&mut self.registered);
        for (clip, held) in registered.iter().enumerate() {
            for membership in held {
                self.add(clip as u32, *membership);
            }
        }
        self.registered = registered;
    }

    /// Adds `clip` to the list of the space `membership` refers to.
    fn add(&mut self, clip: u32, membership: Membership) {
        let (lists, space) = match membership {
            Membership::Uses(space) => (&mut self.users, space),
            Membership::Keeps(space) => (&mut self.keepers, space),
        };
        let space = space as usize;
        if lists.len() <= space {
            lists.resize_with(space + 1, Vec::new);
        }
        lists[space].push(clip);
        self.edges += 1;
    }
}
