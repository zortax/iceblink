//! The references the changed nodes, slots and resources of a commit gain and lose.

use iceblink::{OrderNode, RejectReason};
use smallvec::SmallVec;

use crate::commit::journal::Journal;
use crate::commit::net::{Net, Nets, nets};
use crate::commit::precedence::Earliest;
use crate::commit::validate::in_use::Delta;
use crate::scene::Scene;
use crate::scene::kind::{Kind, ObjectRef};
use crate::scene::names::Names;
use crate::scene::record::{
    ClipRecord, EffectRecord, ImageRecord, PaintRecord, ParamsRecord, ScrollRecord, ShaderRecord,
    TextureRecord, TransformRecord,
};
use crate::scene::slab::Slab;
use crate::scene::stored::Stored;

/// The net changes of every kind of object other values or chunks can refer to.
pub(super) struct Changed<'a> {
    /// The state after the commit.
    scene: &'a Scene,
    transforms: Nets<'a, TransformRecord>,
    scrolls: Nets<'a, ScrollRecord>,
    clips: Nets<'a, ClipRecord>,
    effects: Nets<'a, EffectRecord>,
    paints: Nets<'a, PaintRecord>,
    params: Nets<'a, ParamsRecord>,
    images: Nets<'a, ImageRecord>,
    textures: Nets<'a, TextureRecord>,
    shaders: Nets<'a, ShaderRecord>,
    /// Each order node with an effect before or after one of its changes, with its node before
    /// the commit.
    effect_orders: SmallVec<[(u32, Option<OrderNode>); 4]>,
}

impl<'a> Changed<'a> {
    /// Returns the net changes `journal` holds, against the state after the commit.
    pub(super) fn of(scene: &'a Scene, journal: &'a Journal) -> Self {
        Self {
            scene,
            transforms: nets(&journal.transforms, &scene.transforms),
            scrolls: nets(&journal.scrolls, &scene.scrolls),
            clips: nets(&journal.clips, &scene.clips),
            effects: nets(&journal.effects, &scene.effects),
            paints: nets(&journal.paints, &scene.paints),
            params: nets(&journal.params, &scene.params),
            images: nets(&journal.images, &scene.images),
            textures: nets(&journal.textures, &scene.textures),
            shaders: nets(&journal.shaders, &scene.shaders),
            effect_orders: journal.effect_order_nets(),
        }
    }

    /// Adds the references every changed node gains or loses to `delta`, by slot.
    pub(super) fn count(&self, delta: &mut Delta) {
        let scene = self.scene;

        count(&self.transforms, &scene.transforms, delta);
        count(&self.scrolls, &scene.scrolls, delta);
        count(&self.clips, &scene.clips, delta);
        count(&self.effects, &scene.effects, delta);

        // An order node refers only to its effect.
        for (slot, before) in &self.effect_orders {
            if let Some(before) = before {
                before.names(&mut |object, _| *delta.entry(object).or_default() -= 1);
            }
            if let Some(record) = scene.orders.at(*slot) {
                record
                    .node
                    .names(&mut |object, _| *delta.entry(object).or_default() += 1);
            }
        }
    }

    /// Takes the references to every object the commit created at the slot of a removed one out
    /// of `delta`, so that they keep only the new object in use.
    pub(super) fn forget_replacements(&self, delta: &mut Delta) {
        let scene = self.scene;

        let mut replaced: SmallVec<[ObjectRef; 2]> = SmallVec::new();
        replacements(&self.transforms, Kind::Transform, &mut replaced);
        replacements(&self.scrolls, Kind::Scroll, &mut replaced);
        replacements(&self.clips, Kind::Clip, &mut replaced);
        replacements(&self.effects, Kind::Effect, &mut replaced);
        replacements(&self.paints, Kind::Paint, &mut replaced);
        replacements(&self.params, Kind::Params, &mut replaced);
        replacements(&self.images, Kind::Image, &mut replaced);
        replacements(&self.textures, Kind::Texture, &mut replaced);
        replacements(&self.shaders, Kind::Shader, &mut replaced);
        if replaced.is_empty() {
            return;
        }

        let mut forget = |object: ObjectRef, generation: u32| {
            if replaced.contains(&object) && scene.holds(object, generation) {
                *delta.entry(object).or_default() -= 1;
            }
        };
        mentions(&self.transforms, &scene.transforms, &mut forget);
        mentions(&self.scrolls, &scene.scrolls, &mut forget);
        mentions(&self.clips, &scene.clips, &mut forget);
        mentions(&self.effects, &scene.effects, &mut forget);
        for (slot, _) in &self.effect_orders {
            if let Some(record) = scene.orders.at(*slot) {
                record.node.names(&mut forget);
            }
        }
    }

    /// Notes every removal of an object that anything still refers to, with `delta` holding the
    /// references the commit added and dropped.
    pub(super) fn named(&self, delta: &Delta, earliest: &mut Earliest) {
        named(&self.transforms, Kind::Transform, delta, earliest);
        named(&self.scrolls, Kind::Scroll, delta, earliest);
        named(&self.clips, Kind::Clip, delta, earliest);
        named(&self.effects, Kind::Effect, delta, earliest);
        named(&self.paints, Kind::Paint, delta, earliest);
        named(&self.params, Kind::Params, delta, earliest);
        named(&self.images, Kind::Image, delta, earliest);
        named(&self.textures, Kind::Texture, delta, earliest);
        named(&self.shaders, Kind::Shader, delta, earliest);
    }
}

/// Adds the references each changed node's value gains or loses, by slot.
fn count<R: Stored>(nets: &[Net<'_, R>], slab: &Slab<R>, delta: &mut Delta) {
    for net in nets {
        if let Some(before) = net.before {
            before.names(&mut |object, _| *delta.entry(object).or_default() -= 1);
        }
        if let Some(record) = slab.at(net.slot) {
            record
                .value()
                .names(&mut |object, _| *delta.entry(object).or_default() += 1);
        }
    }
}

/// Adds the objects of `kind` that `nets` replaced to `replaced`.
fn replacements<R: Stored>(
    nets: &[Net<'_, R>],
    kind: Kind,
    replaced: &mut SmallVec<[ObjectRef; 2]>,
) {
    let found = nets.iter().filter(|net| net.replaced());
    replaced.extend(found.map(|net| ObjectRef::new(kind, net.slot)));
}

/// Calls `name` with every object and generation the changed nodes refer to after the commit.
fn mentions<R: Stored>(nets: &[Net<'_, R>], slab: &Slab<R>, name: &mut dyn FnMut(ObjectRef, u32)) {
    for record in nets.iter().filter_map(|net| slab.at(net.slot)) {
        record.value().names(name);
    }
}

/// Notes every removal in `nets` of an object that anything still refers to.
fn named<R: Stored>(nets: &[Net<'_, R>], kind: Kind, delta: &Delta, earliest: &mut Earliest) {
    for net in nets {
        let Some(command) = net.removed else {
            continue;
        };
        let base = net
            .links
            .map_or(0, |links| i64::from(links.refs) + links.users.len() as i64);
        let gained = delta
            .get(&ObjectRef::new(kind, net.slot))
            .copied()
            .unwrap_or(0);
        if base + gained > 0 {
            earliest.note(command, RejectReason::InUse);
        }
    }
}
