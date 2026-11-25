//! The commands of a client commit that the tier reads, applied to the scene and staged.

use iceblink::{
    Animation, AnimationId, AnimationTarget, CommitError, RejectReason, ScrollId, ScrollNode,
};

use crate::commit::apply::Apply;
use crate::commit::finite;
use crate::motion::check::animation_valid;
use crate::tier::View;
use crate::tier::anim::effect::has_param;
use crate::tier::scroll::state::{Geometry, range, shown_offset};
use crate::tier::stage::StagedScroll;

impl Apply<'_> {
    /// Checks an animation against its target. The tier starts it once the commit succeeds.
    pub(crate) fn animate(
        &mut self,
        _id: AnimationId,
        animation: &Animation,
    ) -> Result<(), CommitError> {
        let scene = &*self.scene;
        let live = match animation.target {
            AnimationTarget::Transform(id) => {
                scene.transforms.get(id.slot(), id.generation()).is_some()
            }
            AnimationTarget::Opacity(id) => scene.effects.get(id.slot(), id.generation()).is_some(),
            AnimationTarget::ScrollOffset(id) => {
                scene.scrolls.get(id.slot(), id.generation()).is_some()
            }
            AnimationTarget::Filter { effect, .. } => scene
                .effects
                .get(effect.slot(), effect.generation())
                .is_some(),
        };
        self.require(live, RejectReason::StaleId)?;

        let fits = match animation.target {
            AnimationTarget::Filter {
                effect,
                index,
                param,
            } => scene
                .effects
                .get(effect.slot(), effect.generation())
                .and_then(|record| record.node.filters.get(usize::from(index)))
                .is_some_and(|filter| has_param(filter, param)),
            _ => true,
        };
        self.require(
            fits && animation_valid(animation),
            RejectReason::InvalidItem,
        )?;

        self.journal.tier.animates = true;
        Ok(())
    }

    /// Writes a scroll node's offset as the client's base, which makes the compositor delta zero.
    pub(crate) fn scroll_to_base(
        &mut self,
        id: ScrollId,
        offset: [f32; 2],
    ) -> Result<(), CommitError> {
        self.scroll_to(id, offset)?;
        self.stage_reset(id);
        Ok(())
    }

    /// Writes a scroll node:
    ///
    /// - If the offset equals the base, the compositor delta and the node's motion are kept. A
    ///   node the next frame advances keeps the offset it shows until that frame.
    /// - Otherwise the offset is written as [`Apply::scroll_to_base`] writes it.
    pub(crate) fn scroll_node(
        &mut self,
        id: ScrollId,
        node: ScrollNode,
    ) -> Result<(), CommitError> {
        let Some(staged) = self.staged_scroll(id) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        if node.offset != staged.base {
            self.set_scroll_node(id, node)?;
            self.stage_reset(id);
            return Ok(());
        }

        let offset = match self.scene.scrolls.get(id.slot(), id.generation()) {
            Some(record) if self.advances(id) => record.node.offset,
            _ => self.shown(id, &staged, &node),
        };
        self.set_scroll_node(id, ScrollNode { offset, ..node })
    }

    /// Adopts the compositor's scroll of a node up to `serial`, with `offset` as the new base.
    ///
    /// A node the next frame advances keeps the offset it shows until that frame.
    pub(crate) fn acknowledge_scroll(
        &mut self,
        id: ScrollId,
        serial: u64,
        offset: [f32; 2],
    ) -> Result<(), CommitError> {
        let Some(mut staged) = self.staged_scroll(id) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        self.require(finite::all(&offset), RejectReason::InvalidItem)?;

        let newer = staged
            .acknowledged
            .is_none_or(|acknowledged| serial > acknowledged);
        if newer && !staged.reset {
            let at = self
                .scene
                .tier
                .scrolls
                .nodes
                .get(&id.slot())
                .filter(|state| state.id == id)
                .map_or([0.0; 2], |state| state.serials.delta_at(serial));
            for (delta, (at, adopted)) in
                staged.delta.iter_mut().zip(at.iter().zip(&staged.adopted))
            {
                *delta -= at - adopted;
            }
            staged.adopted = at;
            staged.acknowledged = Some(serial);
        }

        let Some(record) = self.scene.scrolls.get(id.slot(), id.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        let node = record.node.clone();
        let max = range(&node);
        staged.base = [0, 1].map(|axis| offset[axis].clamp(0.0, max[axis]));
        self.journal.tier.set_scroll(staged);

        let shown = self.shown(id, &staged, &node);
        // An adoption that moves no pixel writes nothing, so the commit keeps its declared damage.
        // The next frame moves a node in motion, with the input that waits for it.
        if shown == node.offset || self.advances(id) {
            return Ok(());
        }
        self.scroll_to(id, shown)
    }

    /// Reports whether the next frame advances node `id`, which then shows its base and delta.
    fn advances(&self, id: ScrollId) -> bool {
        let scrolls = &self.scene.tier.scrolls;
        scrolls.moving.contains(&id.slot())
            && scrolls
                .nodes
                .get(&id.slot())
                .is_some_and(|state| state.id == id)
    }

    /// Returns the offset `node` shows with a staged base and delta.
    fn shown(&self, id: ScrollId, staged: &StagedScroll, node: &ScrollNode) -> [f32; 2] {
        let view = View {
            transforms: &self.scene.transforms,
            scrolls: &self.scene.scrolls,
            effects: &self.scene.effects,
        };
        let scale = Geometry::scale_of(&view, node);
        let origin = self
            .scene
            .tier
            .scrolls
            .nodes
            .get(&id.slot())
            .filter(|state| state.id == id && !staged.reset)
            .and_then(|state| state.programmatic)
            .unwrap_or(staged.base);
        shown_offset(origin, staged.delta, range(node), scale)
    }

    /// Returns the base and delta of a node as the commands applied so far leave them.
    fn staged_scroll(&self, id: ScrollId) -> Option<StagedScroll> {
        if let Some(staged) = self.journal.tier.scroll(id) {
            return Some(staged);
        }

        let record = self.scene.scrolls.get(id.slot(), id.generation())?;
        let state = self
            .scene
            .tier
            .scrolls
            .nodes
            .get(&id.slot())
            .filter(|state| state.id == id);
        Some(StagedScroll {
            id,
            base: state.map_or(record.node.offset, |state| state.base),
            delta: state.map_or([0.0; 2], |state| state.delta),
            reset: false,
            acknowledged: None,
            adopted: [0.0; 2],
        })
    }

    /// Stages a node's written offset as its base, with no compositor delta.
    ///
    /// A node the compositor never moved needs nothing staged, since its base is the offset
    /// written.
    fn stage_reset(&mut self, id: ScrollId) {
        let moved = self
            .scene
            .tier
            .scrolls
            .nodes
            .get(&id.slot())
            .is_some_and(|state| state.id == id);
        if !moved && self.journal.tier.scroll(id).is_none() {
            return;
        }

        let Some(record) = self.scene.scrolls.get(id.slot(), id.generation()) else {
            return;
        };
        self.journal.tier.set_scroll(StagedScroll {
            id,
            base: record.node.offset,
            delta: [0.0; 2],
            reset: true,
            acknowledged: None,
            adopted: [0.0; 2],
        });
    }
}
