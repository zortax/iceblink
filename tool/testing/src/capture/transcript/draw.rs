//! The draw sequence: every batch with the instances it draws, in order.

use iceblink::{Batch, BatchKind, Frame, Instance, ItemSource, Lane, LoweredChunk};

use crate::capture::mirror::Mirror;
use crate::capture::text::Writer;
use crate::capture::transcript::instance::{self, device, resolve};
use crate::capture::transcript::plans;

/// A frame and what the renderer holds once its work is applied.
#[derive(Clone, Copy)]
struct Held<'a> {
    frame: &'a Frame,
    mirror: &'a Mirror,
}

/// Writes every batch, with its instances or its object nested beneath it.
pub(crate) fn batches(writer: &mut Writer, frame: &Frame, mirror: &Mirror) {
    let held = Held { frame, mirror };
    writer.nested(&format!("batches {}", frame.batches.len()), |writer| {
        for batch in &frame.batches {
            one(writer, held, batch);
        }
    });
}

fn one(writer: &mut Writer, held: Held<'_>, batch: &Batch) {
    let Held { frame, mirror } = held;
    let lanes = &frame.lanes;
    let ink = device(batch.ink);
    let range = format!("{}..{}", batch.range.start, batch.range.end);
    match batch.kind {
        BatchKind::Quads => {
            let header = format!("quads {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.quads,
                batch,
                |c| &c.quads,
                |m, q, _| instance::quad(m, q),
            );
        }
        BatchKind::ShaderQuads { shader, params } => {
            let header = format!("shader_quads #{shader} params=#{params} {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.shader_quads,
                batch,
                |c| &c.shader_quads,
                |m, q, _| instance::shader_quad(m, q),
            );
        }
        BatchKind::Shadows => {
            let header = format!("shadows {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.shadows,
                batch,
                |c| &c.shadows,
                |m, s, _| instance::shadow(m, s),
            );
        }
        BatchKind::Lines => {
            let header = format!("lines {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.lines,
                batch,
                |c| &c.lines,
                |m, l, _| instance::line(m, l),
            );
        }
        BatchKind::Mono { page } => {
            let header = format!("mono page{page} {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.mono,
                batch,
                |c| &c.mono,
                |m, s, _| instance::coverage(m, "mono", s),
            );
        }
        BatchKind::Subpixel { page } => {
            let header = format!("subpixel page{page} {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.subpixel,
                batch,
                |c| &c.subpixel,
                |m, s, _| instance::coverage(m, "subpixel", s),
            );
        }
        BatchKind::Color { page } => {
            let header = format!("color page{page} {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.color,
                batch,
                |c| &c.color,
                |m, s, _| instance::color(m, s),
            );
        }
        BatchKind::Marks => {
            let header = format!("marks {range} ink={ink}");
            each(
                writer,
                &header,
                held,
                &lanes.marks,
                batch,
                |c| &c.marks,
                |m, k, at| {
                    let mut line = instance::marks(m, k);
                    line.push_str(&plans::bin(frame, at));
                    line
                },
            );
        }
        BatchKind::Paths { pass } => plans::pass(writer, frame, mirror, pass),
        BatchKind::External(index) => match frame.externals.get(index as usize) {
            Some(external) => writer.line(&instance::external(mirror, external)),
            None => writer.line(&format!("external {index} <missing>")),
        },
        BatchKind::Backdrop(index) => match frame.backdrops.get(index as usize) {
            Some(backdrop) => writer.line(&instance::backdrop(mirror, backdrop)),
            None => writer.line(&format!("backdrop {index} <missing>")),
        },
        BatchKind::GroupBegin(index) => match frame.groups.get(index as usize) {
            Some(group) => writer.line(&instance::group_begin(mirror, index, group)),
            None => writer.line(&format!("group_begin {index} <missing>")),
        },
        BatchKind::GroupEnd(index) => writer.line(&format!("group_end {index}")),
    }
}

/// Writes `header` and one line per instance of the batch, rendered by `line` with the source's
/// position in the lane.
fn each<T: Instance>(
    writer: &mut Writer,
    header: &str,
    held: Held<'_>,
    lane: &Lane<T>,
    batch: &Batch,
    array: fn(&LoweredChunk) -> &Vec<T>,
    line: impl Fn(&Mirror, &T, u32) -> String,
) {
    let Held { frame, mirror } = held;
    writer.nested(header, |writer| {
        let instances: Vec<ItemSource> = lane.instances().collect();
        for at in batch.range.clone() {
            let Some(source) = instances.get(at as usize) else {
                writer.line(&format!("<missing source {at}>"));
                continue;
            };
            match resolve(frame, mirror, lane, *source, array) {
                Ok(instance) => writer.line(&line(mirror, &instance, at)),
                Err(missing) => writer.line(&missing),
            }
        }
    });
}
