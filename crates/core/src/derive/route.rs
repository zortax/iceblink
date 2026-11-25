//! Finding the outputs that draw a piece of content, and sending damage to them.

use iceblink::{ChunkKey, DeviceRect, OutputId};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::commit::journal::Journal;
use crate::derive::spread::{Spread, upwards};
use crate::output::Outputs;
use crate::scene::Scene;

/// An output that draws some content, and the spread of the groups around the content.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Route {
    pub(crate) output: OutputId,
    pub(crate) spread: Spread,
}

/// The outputs whose root belongs to one client, by the slot of the root.
pub(crate) struct Routes {
    roots: Roots,
    all: SmallVec<[OutputId; 2]>,
}

/// The output each root order node is drawn by, by the root's slot.
#[derive(Clone, Debug, Default, PartialEq)]
struct Roots(SmallVec<[(u32, OutputId); 2]>);

impl Roots {
    /// Returns the output whose root is the order node at `slot`.
    fn get(&self, slot: u32) -> Option<OutputId> {
        self.0
            .iter()
            .find(|(root, _)| *root == slot)
            .map(|&(_, output)| output)
    }
}

impl Routes {
    /// Collects the outputs whose root is an order node of `scene`'s client.
    pub(crate) fn of(scene: &Scene, outputs: &Outputs) -> Self {
        let mut roots = Roots::default();
        let mut all = SmallVec::new();
        for (&id, output) in outputs {
            if let Some((client, root)) = output.root
                && client == scene.client
            {
                roots.0.push((root.slot(), id));
                all.push(id);
            }
        }
        Self { roots, all }
    }

    /// Returns the route of the content of the order node at `slot`, if an output draws it.
    pub(crate) fn of_node(&self, scene: &Scene, slot: u32) -> Option<Route> {
        let mut spread = Spread::default();
        for node in upwards(scene, slot) {
            spread = spread.and(Spread::of_node(scene, node));
            if let Some(output) = self.roots.get(node) {
                return Some(Route { output, spread });
            }
        }
        None
    }

    /// Returns the route of the order node at `slot` itself, if an output draws it.
    ///
    /// The spread leaves out the node's own group, whose written region is the node's ink.
    pub(crate) fn of_group(&self, scene: &Scene, slot: u32) -> Option<Route> {
        if let Some(output) = self.roots.get(slot) {
            let spread = Spread::default();
            return Some(Route { output, spread });
        }
        let parent = scene.node_placements.first(slot)?;
        self.of_node(scene, parent)
    }

    /// Returns the route of the chunk `key`, if an output draws it.
    pub(crate) fn of_chunk(&self, scene: &Scene, key: ChunkKey) -> Option<Route> {
        let parent = scene.placements.first(&key)?;
        self.of_node(scene, parent)
    }

    /// Adds `rect` to the damage of every output the client draws into.
    pub(crate) fn damage_all(&self, outputs: &mut Outputs, rect: DeviceRect) {
        for id in &self.all {
            if let Some(output) = outputs.get_mut(id) {
                output.damage(rect);
            }
        }
    }
}

/// Routes of order nodes found while the order tree, its effects and the outputs' roots stay as
/// they are.
#[derive(Debug, Default)]
pub(crate) struct RouteCache {
    /// The output that draws each node seen, and whether no group around it spreads.
    found: FxHashMap<u32, Option<(OutputId, bool)>>,
    /// The roots the routes were found for.
    roots: Roots,
}

impl RouteCache {
    /// Forgets every route when `journal` changed the order tree, an effect or a root, or when
    /// `routes` start at other roots.
    pub(crate) fn keep_for(&mut self, routes: &Routes, journal: &Journal) {
        let changed = !journal.orders.is_empty()
            || !journal.effects.is_empty()
            || !journal.roots.is_empty()
            || self.roots != routes.roots;
        if changed {
            self.found.clear();
            self.roots.clone_from(&routes.roots);
        }
    }

    /// Returns the route of the chunk `key`, as [`Routes::of_chunk`] does.
    pub(crate) fn of_chunk(
        &mut self,
        routes: &Routes,
        scene: &Scene,
        key: ChunkKey,
    ) -> Option<Route> {
        let parent = scene.placements.first(&key)?;
        let (output, plain) = self.find(routes, scene, parent)?;
        if plain {
            let spread = Spread::default();
            return Some(Route { output, spread });
        }
        routes.of_node(scene, parent)
    }

    /// Returns the output that draws the order node at `slot` and whether no group around it
    /// spreads, remembering the answer for the node and the ancestors it passed.
    fn find(&mut self, routes: &Routes, scene: &Scene, slot: u32) -> Option<(OutputId, bool)> {
        let mut path: SmallVec<[u32; 16]> = SmallVec::new();
        let mut above = None;
        for node in upwards(scene, slot) {
            if let Some(&known) = self.found.get(&node) {
                above = Some(known);
                break;
            }
            path.push(node);
            if let Some(output) = routes.roots.get(node) {
                above = Some(Some((output, true)));
                break;
            }
        }
        // A walk that ended without an output leaves every node on it undrawn.
        let mut found = above.flatten();
        for &node in path.iter().rev() {
            found = found.map(|(output, plain)| {
                (
                    output,
                    plain && Spread::of_node(scene, node) == Spread::default(),
                )
            });
            self.found.insert(node, found);
        }
        found
    }
}

/// Adds `rect`, grown by the route's spread, to the damage of the route's output, when there is
/// one.
pub(crate) fn damage(outputs: &mut Outputs, route: Option<Route>, rect: Option<DeviceRect>) {
    if let (Some(route), Some(rect)) = (route, rect)
        && let Some(output) = outputs.get_mut(&route.output)
    {
        output.damage(route.spread.grow(rect));
    }
}
