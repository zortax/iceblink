//! The items of chunk 1 of the scene, one or more of every kind.

mod boxes;
mod glyphs;
mod pictures;
mod shapes;

use iceblink::{
    ChunkRecorder, ClipRef, ClipShape, FontId, LocalClip, LocalParent, MaskMode, PaintKind,
    PaintRef, ROUND, SpaceRef,
};

use super::nodes::Nodes;
use super::resources::Resources;

/// The fonts the scene's glyph runs are set in.
#[derive(Clone, Copy)]
pub(super) struct Fonts {
    /// A Latin text face.
    pub(super) latin: FontId,
    /// A face with colour layers.
    pub(super) color: FontId,
}

/// The chunk's references to the nodes, paints and clips its items draw with.
struct Refs {
    gradient: PaintRef,
    sides: PaintRef,
    sticky: SpaceRef,
    fixed: SpaceRef,
    turned: SpaceRef,
    /// The rounded clip node.
    clip: ClipRef,
    /// A rounded local clip inside the rounded clip node.
    local: ClipRef,
    /// An alpha mask local clip inside `local`.
    nested: ClipRef,
}

/// Records the items into `recorder`.
pub(super) fn record(
    recorder: &mut ChunkRecorder<'_>,
    nodes: &Nodes,
    resources: &Resources,
    fonts: Fonts,
) {
    let refs = refs(recorder, nodes, resources);
    boxes::record(recorder, &refs, resources);
    glyphs::record(recorder, &refs, fonts);
    pictures::image(recorder, &refs, resources.image);
    shapes::record(recorder, &refs);
    pictures::external(recorder, resources.texture);
    pictures::backdrop(recorder);
}

/// Adds the chunk's references, and the local clips.
fn refs(recorder: &mut ChunkRecorder<'_>, nodes: &Nodes, resources: &Resources) -> Refs {
    let gradient = recorder.paint_ref(PaintKind::GRADIENT, resources.gradient);
    let sides = recorder.paint_ref(PaintKind::SIDES, resources.sides);
    let sticky = recorder.space_ref(nodes.sticky);
    let fixed = recorder.space_ref(nodes.fixed);
    let turned = recorder.space_ref(nodes.turned);
    let clip = recorder.clip_ref(resources.rounded);
    let local = recorder.local_clip(LocalClip {
        parent: LocalParent::Node(resources.rounded),
        shape: ClipShape::RoundedRect {
            rect: [2.0, 2.0, 80.0, 50.0],
            radii: [3.0; 8],
            shapes: ROUND,
        },
    });
    let nested = recorder.local_clip(LocalClip {
        parent: LocalParent::Local(0),
        shape: ClipShape::Mask {
            image: resources.image,
            rect: [0.0, 0.0, 40.0, 40.0],
            source: [0.5, 0.5, 15.0, 15.0],
            mode: MaskMode::ALPHA,
        },
    });

    Refs {
        gradient,
        sides,
        sticky,
        fixed,
        turned,
        clip,
        local,
        nested,
    }
}
