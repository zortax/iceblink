//! Checking the chunks a commit changed, and what they refer to before and after.

use iceblink::FontRegistry;
use iceblink::{AnyId, ChunkKey, RecordedChunk, RejectReason};
use smallvec::SmallVec;

use crate::commit::journal::{ChunkChange, Journal};
use crate::commit::precedence::Earliest;
use crate::commit::validate::items::{check_chunk, content_deps};
use crate::commit::validate::{ChunkNet, ChunkNets};
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::kind::{Kind, ObjectRef};
use crate::small::SmallMap;

/// The most changed chunk keys whose states are kept without allocating.
const INLINE: usize = 16;

/// What the journal says about one chunk key.
struct KeyState<'a> {
    /// The record the key held before the commit, once a replacement revealed it.
    original: Option<Option<&'a ChunkRecord>>,
    /// The command of the last recording, when the key's final record is new.
    recorded: Option<u32>,
    /// Whether a command wrote the key's props.
    props: bool,
}

/// Checks every chunk the commit recorded, notes each broken rule at its recording in `earliest`,
/// and adds what each changed key refers to before and after the commit to `nets`.
pub(super) fn nets(
    scene: &Scene,
    journal: &Journal,
    fonts: &dyn FontRegistry,
    nets: &mut ChunkNets,
    earliest: &mut Earliest,
) {
    let mut order: SmallVec<[ChunkKey; INLINE]> = SmallVec::new();
    let mut states: SmallMap<ChunkKey, KeyState<'_>, INLINE> = SmallMap::default();
    for change in &journal.chunks {
        let key = match change {
            ChunkChange::Replaced { key, .. }
            | ChunkChange::Offset { key, .. }
            | ChunkChange::Props { key, .. } => *key,
        };
        let state = states.get_or_insert_with(key, || {
            order.push(key);
            KeyState {
                original: None,
                recorded: None,
                props: false,
            }
        });

        match change {
            ChunkChange::Replaced {
                command, before, ..
            } => {
                if state.original.is_none() {
                    state.original = Some(journal.before(*before));
                }
                state.recorded = scene.chunks.contains_key(&key).then_some(*command);
            }
            ChunkChange::Props { .. } => state.props = true,
            ChunkChange::Offset { .. } => {}
        }
    }

    for key in order {
        let Some(state) = states.get(&key) else {
            continue;
        };
        let original = state.original.unwrap_or_else(|| scene.chunks.get(&key));
        let before = original.map(|record| {
            let mut list = nets.list();
            list.extend_from_slice(&record.derived.deps);
            list
        });

        let after = match scene.chunks.get(&key) {
            None => None,
            Some(record) if state.recorded.is_some() || state.props => {
                let mut deps = nets.list();
                let broken = check_chunk(scene, &record.recorded, fonts, &mut deps);
                // Only a recording is judged by the chunk's rules. A chunk that only took new
                // props refers to what it referred to before, which the in-use check judges.
                if let Some(command) = state.recorded {
                    if let Err(reason) = check_refs(scene, &record.recorded) {
                        earliest.note(command, reason);
                    }
                    if let Some(reason) = broken {
                        earliest.note(command, reason);
                    }
                }
                Some(deps)
            }
            Some(_) => before.as_ref().map(|before| {
                let mut list = nets.list();
                list.extend_from_slice(before);
                list
            }),
        };

        let renamed = match scene.chunks.get(&key) {
            Some(record) if state.props && state.recorded.is_none() => {
                renamed(scene, journal, &record.recorded, fonts)
            }
            _ => SmallVec::new(),
        };

        nets.nets.push(ChunkNet {
            key,
            before,
            after,
            recorded: state.recorded.is_some(),
            renamed,
        });
    }
}

/// Returns the objects that `chunk` names only through its props, by the id of an object the
/// commit created at the slot of a removed one.
fn renamed(
    scene: &Scene,
    journal: &Journal,
    chunk: &RecordedChunk,
    fonts: &dyn FontRegistry,
) -> SmallVec<[ObjectRef; 2]> {
    // A live id at a slot the commit freed names the object created after the removal.
    let replacement = |&(object, generation): &(ObjectRef, u32)| {
        scene.holds(object, generation)
            && journal.freed.iter().any(|id| match (*id, object.kind) {
                (AnyId::Transform(id), Kind::Transform) => id.slot() == object.slot,
                (AnyId::Clip(id), Kind::Clip) => id.slot() == object.slot,
                _ => false,
            })
    };

    let props = chunk.props;
    let space = ObjectRef::new(Kind::Transform, props.space.slot());
    let clip = props
        .clip
        .map(|clip| (ObjectRef::new(Kind::Clip, clip.slot()), clip.generation()));
    let mut named: SmallVec<[ObjectRef; 2]> = [Some((space, props.space.generation())), clip]
        .into_iter()
        .flatten()
        .filter(replacement)
        .map(|(object, _)| object)
        .collect();
    if named.is_empty() {
        return named;
    }

    let mut contents = Vec::new();
    content_deps(scene, chunk, fonts, &mut contents);
    named.retain(|object| contents.binary_search(object).is_err());
    named
}

/// Checks that every id a chunk's slots were taken from is live with its current generation.
fn check_refs(scene: &Scene, chunk: &RecordedChunk) -> Result<(), RejectReason> {
    let live = |id: &AnyId| match *id {
        AnyId::Transform(id) => scene.transforms.get(id.slot(), id.generation()).is_some(),
        AnyId::Scroll(id) => scene.scrolls.get(id.slot(), id.generation()).is_some(),
        AnyId::Clip(id) => scene.clips.get(id.slot(), id.generation()).is_some(),
        AnyId::Effect(id) => scene.effects.get(id.slot(), id.generation()).is_some(),
        AnyId::Order(id) => scene.orders.get(id.slot(), id.generation()).is_some(),
        AnyId::Paint(id) => scene.paints.get(id.slot(), id.generation()).is_some(),
        AnyId::Params(id) => scene.params.get(id.slot(), id.generation()).is_some(),
        AnyId::Image(id) => scene.images.get(id.slot(), id.generation()).is_some(),
        AnyId::Texture(id) => scene.textures.get(id.slot(), id.generation()).is_some(),
        AnyId::Shader(id) => scene.shaders.get(id.slot(), id.generation()).is_some(),
        AnyId::Animation(_) => false,
    };

    if chunk.refs.iter().all(live) {
        Ok(())
    } else {
        Err(RejectReason::StaleId)
    }
}
