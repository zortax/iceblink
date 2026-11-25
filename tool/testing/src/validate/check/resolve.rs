//! Lane sources resolved to the instances they draw.

use iceblink::{
    ColorSprite, FrameMarks, FrameShadow, Instance, ItemSource, Lane, Line, LoweredChunk, Quad,
    ShaderQuad, Sprite,
};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::rects::finite;
use crate::validate::known::Lookup;

/// A drawn instance at its final position, with the resident chunk it comes from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Resolved<'a, T> {
    pub(crate) instance: T,
    /// The resident chunk, or `None` for a transient instance.
    pub(crate) chunk: Option<&'a LoweredChunk>,
}

/// The resolved instance of each lane source.
///
/// An entry is `None` when its source refers to no instance, or to a revision the frames seen so
/// far do not hold.
pub(crate) type Sources<'a, T> = Vec<Option<Resolved<'a, T>>>;

/// The sources of every lane, resolved.
pub(crate) struct Lanes<'a> {
    pub(crate) quads: Sources<'a, Quad>,
    pub(crate) shader_quads: Sources<'a, ShaderQuad>,
    pub(crate) shadows: Sources<'a, FrameShadow>,
    pub(crate) lines: Sources<'a, Line>,
    pub(crate) mono: Sources<'a, Sprite>,
    pub(crate) subpixel: Sources<'a, Sprite>,
    pub(crate) color: Sources<'a, ColorSprite>,
    pub(crate) marks: Sources<'a, FrameMarks>,
}

/// Resolves every lane's sources, reporting those that refer to no instance.
pub(crate) fn lanes<'a>(inspect: &mut Inspect<'a>) -> Lanes<'a> {
    drawn(inspect);

    let lanes = &inspect.frame.lanes;
    Lanes {
        quads: lane(inspect, "quads", &lanes.quads, |chunk| &chunk.quads),
        shader_quads: lane(inspect, "shader quads", &lanes.shader_quads, |chunk| {
            &chunk.shader_quads
        }),
        shadows: lane(inspect, "shadows", &lanes.shadows, |chunk| &chunk.shadows),
        lines: lane(inspect, "lines", &lanes.lines, |chunk| &chunk.lines),
        mono: lane(inspect, "mono", &lanes.mono, |chunk| &chunk.mono),
        subpixel: lane(inspect, "subpixel", &lanes.subpixel, |chunk| {
            &chunk.subpixel
        }),
        color: lane(inspect, "color", &lanes.color, |chunk| &chunk.color),
        marks: lane(inspect, "marks", &lanes.marks, |chunk| &chunk.marks),
    }
}

/// Checks that every drawn chunk is resident and moved by a finite offset.
fn drawn(inspect: &mut Inspect<'_>) {
    for (index, drawn) in inspect.frame.drawn.iter().enumerate() {
        if inspect.known.resident.get(drawn.revision).is_missing() {
            inspect.push(
                Rule::Sources,
                format!(
                    "drawn chunk {index} names revision {}, which is not resident",
                    drawn.revision
                ),
            );
        }
        if !finite(&drawn.offset) {
            inspect.push(
                Rule::Values,
                format!("drawn chunk {index} has offset {:?}", drawn.offset),
            );
        }
    }
}

fn lane<'a, T: Instance>(
    inspect: &mut Inspect<'a>,
    name: &str,
    lane: &'a Lane<T>,
    array: fn(&LoweredChunk) -> &Vec<T>,
) -> Sources<'a, T> {
    for (position, run) in lane.sources.iter().enumerate() {
        if run.len == 0 {
            inspect.push(
                Rule::Sources,
                format!("{name} run {position} draws no instance"),
            );
        }
    }
    let mut resolved = Vec::with_capacity(lane.drawn());
    for (position, source) in lane.instances().enumerate() {
        let at = || format!("{name} instance {position}");
        resolved.push(one(inspect, lane, source, array, at));
    }
    resolved
}

fn one<'a, T: Instance>(
    inspect: &mut Inspect<'a>,
    lane: &'a Lane<T>,
    source: ItemSource,
    array: fn(&LoweredChunk) -> &Vec<T>,
    at: impl Fn() -> String,
) -> Option<Resolved<'a, T>> {
    if source.is_transient() {
        let Some(instance) = lane.items.get(source.index as usize) else {
            inspect.push(
                Rule::Sources,
                format!(
                    "{} names item {} of {}",
                    at(),
                    source.index,
                    lane.items.len()
                ),
            );
            return None;
        };
        return Some(Resolved {
            instance: *instance,
            chunk: None,
        });
    }

    let frame = inspect.frame;
    let Some(drawn) = frame.drawn.get(source.chunk as usize) else {
        inspect.push(
            Rule::Sources,
            format!(
                "{} names drawn chunk {} of {}",
                at(),
                source.chunk,
                frame.drawn.len()
            ),
        );
        return None;
    };

    let chunk = match inspect.known.resident.get(drawn.revision) {
        Lookup::Found(chunk) => chunk.as_ref(),
        // The drawn chunk itself is reported once.
        Lookup::Missing | Lookup::Unknown => return None,
    };
    let instances = array(chunk);
    let Some(instance) = instances.get(source.index as usize) else {
        inspect.push(
            Rule::Sources,
            format!(
                "{} names instance {} of revision {}, which has {}",
                at(),
                source.index,
                drawn.revision,
                instances.len()
            ),
        );
        return None;
    };
    Some(Resolved {
        instance: instance.moved(drawn.offset),
        chunk: Some(chunk),
    })
}
