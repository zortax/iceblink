//! Checking the items of a recorded chunk, and collecting the objects they refer to.

mod kinds;
mod marks;
mod refs;

use iceblink::FontRegistry;
use iceblink::{
    ArenaRange, ClipShape, Filter, LocalParent, RecordedChunk, RejectReason, ShaderMode,
};

use crate::commit::finite;
use crate::commit::precedence::Worst;
use crate::scene::Scene;
use crate::scene::kind::{Kind, ObjectRef};

/// Checks one recorded chunk against the state after the commit, and fills `deps` with every
/// object it refers to, each once, live or not.
///
/// Returns the broken rule that comes first in rule order, or `None` for a well-formed chunk:
/// `StaleId` for a typed id of the props, the local clips or the filter chains that is not live,
/// `PaintKindChanged` for a paint reference of the wrong kind, `InvalidShader` for a shader in
/// the wrong mode, and `InvalidItem` for every other rule.
pub(crate) fn check_chunk(
    scene: &Scene,
    chunk: &RecordedChunk,
    fonts: &dyn FontRegistry,
    deps: &mut Vec<ObjectRef>,
) -> Option<RejectReason> {
    let mut check = Check {
        scene,
        chunk,
        deps,
        worst: Worst::default(),
    };

    check.valid(sequential(chunk));
    check.props();
    check.contents(fonts);

    let worst = check.worst;

    deps.sort_unstable();
    deps.dedup();
    worst.reason()
}

/// Fills `deps` with every object a chunk refers to other than through its props, each once.
pub(crate) fn content_deps(
    scene: &Scene,
    chunk: &RecordedChunk,
    fonts: &dyn FontRegistry,
    deps: &mut Vec<ObjectRef>,
) {
    let mut check = Check {
        scene,
        chunk,
        deps,
        worst: Worst::default(),
    };
    check.contents(fonts);

    deps.sort_unstable();
    deps.dedup();
}

/// A check of one recorded chunk, which notes the broken rules and collects the objects the chunk
/// refers to.
struct Check<'a> {
    scene: &'a Scene,
    chunk: &'a RecordedChunk,
    deps: &'a mut Vec<ObjectRef>,
    worst: Worst,
}

impl Check<'_> {
    /// Checks everything in the chunk but its props and the order of its ops.
    fn contents(&mut self, fonts: &dyn FontRegistry) {
        self.local_clips();
        self.filters();
        if let Some(hint) = self.chunk.hint {
            self.finite(&hint);
        }
        self.items(fonts);
    }

    fn props(&mut self) {
        let props = self.chunk.props;
        let scene = self.scene;

        self.deps
            .push(ObjectRef::new(Kind::Transform, props.space.slot()));
        self.live(
            scene
                .transforms
                .get(props.space.slot(), props.space.generation())
                .is_some(),
        );
        if let Some(clip) = props.clip {
            self.deps.push(ObjectRef::new(Kind::Clip, clip.slot()));
            self.live(scene.clips.get(clip.slot(), clip.generation()).is_some());
        }
    }

    fn local_clips(&mut self) {
        for (index, clip) in self.chunk.local_clips.iter().enumerate() {
            match clip.parent {
                LocalParent::Node(id) => {
                    self.deps.push(ObjectRef::new(Kind::Clip, id.slot()));
                    self.live(self.scene.clips.get(id.slot(), id.generation()).is_some());
                }
                LocalParent::Local(parent) => self.valid((parent as usize) < index),
                LocalParent::Chunk | LocalParent::None => {}
            }
            match &clip.shape {
                ClipShape::RoundedRect {
                    rect,
                    radii,
                    shapes,
                } => self.finite(rect.iter().chain(radii).chain(shapes)),
                ClipShape::Mask {
                    image,
                    rect,
                    source,
                    mode,
                } => {
                    self.finite(rect.iter().chain(source));
                    self.valid(mode.is_valid());
                    self.deps.push(ObjectRef::new(Kind::Image, image.slot()));
                    let live = self.scene.images.get(image.slot(), image.generation());
                    self.live(live.is_some());
                }
            }
        }
    }

    fn filters(&mut self) {
        for chain in &self.chunk.filters {
            self.valid(finite::filters(chain));
            for filter in chain.iter() {
                if let Filter::Shader { shader, params, .. } = filter {
                    let scene = self.scene;
                    self.live(
                        scene
                            .shaders
                            .get(shader.slot(), shader.generation())
                            .is_some(),
                    );
                    self.live(
                        scene
                            .params
                            .get(params.slot(), params.generation())
                            .is_some(),
                    );
                    self.shader(shader.slot(), &[ShaderMode::FILTER]);
                    self.deps.push(ObjectRef::new(Kind::Params, params.slot()));
                }
            }
        }
    }

    /// Notes `StaleId` unless a typed id of the chunk is `live`.
    fn live(&mut self, live: bool) {
        self.worst.require(live, RejectReason::StaleId);
    }

    /// Notes `InvalidItem` unless `holds`.
    fn valid(&mut self, holds: bool) {
        self.worst.require(holds, RejectReason::InvalidItem);
    }

    fn finite<'a>(&mut self, values: impl IntoIterator<Item = &'a f32>) {
        self.valid(finite::all(values));
    }
}

/// The number of item kinds.
const KINDS: usize = 10;

/// Reports whether the ops of `chunk` refer to every entry of each item array once, in array
/// order.
fn sequential(chunk: &RecordedChunk) -> bool {
    let mut next = [0u32; KINDS];
    for op in &chunk.ops {
        let Some(count) = next.get_mut(op.kind.0 as usize) else {
            return false;
        };
        if op.index != *count {
            return false;
        }
        *count += 1;
    }

    // In the order of the `ItemKind` values.
    let lens: [usize; KINDS] = [
        chunk.quads.len(),
        chunk.shader_quads.len(),
        chunk.shadows.len(),
        chunk.lines.len(),
        chunk.glyph_runs.len(),
        chunk.images.len(),
        chunk.marks.len(),
        chunk.paths.len(),
        chunk.externals.len(),
        chunk.backdrops.len(),
    ];
    next.iter()
        .zip(lens)
        .all(|(&count, len)| count as usize == len)
}

/// Returns the indices of `range` if they lie inside an arena of `len` values.
fn range(range: ArenaRange, len: usize) -> Option<core::ops::Range<usize>> {
    let end = u64::from(range.start) + u64::from(range.len);
    (end <= len as u64).then(|| range.indices())
}
