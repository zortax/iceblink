//! Applying one command to a draft.

mod chunks;
mod nodes;
mod order;
mod remove;
mod resources;

use iceblink::{Command, OrderEntry, RecordedChunk, RejectReason};

use crate::naive::commit::draft::Draft;

/// Applies `command`, the command at `index`, and fails with the rule it breaks.
pub(crate) fn command(
    draft: &mut Draft<'_>,
    recorded: &mut [Option<RecordedChunk>],
    entries: &[OrderEntry],
    command: Command,
    index: u32,
) -> Result<(), RejectReason> {
    draft.placing.note(&command, index, entries);
    match command {
        Command::CreateTransform(id, node) => nodes::create_transform(draft, id, node),
        Command::SetTransform(id, local) => nodes::set_transform(draft, id, local),
        Command::SetTransformNode(id, node) => nodes::set_transform_node(draft, id, node),
        Command::CreateScroll(id, node) => nodes::create_scroll(draft, id, node),
        Command::SetScrollNode(id, node) => nodes::set_scroll_node(draft, id, node),
        Command::ScrollTo(id, offset) | Command::AcknowledgeScroll { id, offset, .. } => {
            nodes::scroll_to(draft, id, offset)
        }
        Command::CreateClip(id, node) => nodes::create_clip(draft, id, node),
        Command::SetClip(id, node) => nodes::set_clip(draft, id, node),
        Command::CreateEffect(id, node) => nodes::create_effect(draft, id, node),
        Command::SetEffect(id, node) => nodes::set_effect(draft, id, node),
        Command::CreatePaint(id, paint) => nodes::create_paint(draft, id, paint),
        Command::SetPaint(id, paint) => nodes::set_paint(draft, id, paint),
        Command::CreateParams(id, block) => nodes::create_params(draft, id, block),
        Command::SetParams(id, block) => nodes::set_params(draft, id, block),
        Command::CreateOrder(id, node) => order::create(draft, id, node),
        Command::SetOrderEffect(id, effect) => order::set_effect(draft, id, effect),
        Command::Splice {
            node,
            at,
            remove,
            insert,
        } => {
            let insert = entries
                .get(insert.indices())
                .ok_or(RejectReason::InvalidItem)?;
            order::splice(draft, node, at, remove, insert)
        }
        Command::SetRoot(output, root) => order::set_root(draft, output, root),
        Command::Record(chunk) => {
            let chunk = recorded
                .get_mut(chunk as usize)
                .and_then(Option::take)
                .ok_or(RejectReason::InvalidItem)?;
            chunks::record(draft, chunk, index)
        }
        Command::SetProps(key, props) => chunks::set_props(draft, key, props),
        Command::SetOffset(key, offset) => chunks::set_offset(draft, key, offset),
        Command::RemoveChunk(key) => chunks::remove(draft, key),
        Command::CreateImage(id, desc) => resources::create_image(draft, id, desc),
        Command::WriteImage(id, write) => resources::write_image(draft, id, &write),
        Command::CreateTexture(id, desc) => resources::create_texture(draft, id, desc),
        Command::TextureFrame(id) => resources::texture_frame(draft, id),
        Command::CreateShader(id, desc) => resources::create_shader(draft, id, desc),
        Command::Animate(..) => Err(RejectReason::Unsupported),
        // No animation ever starts, so every animation id names one that ended.
        Command::CancelAnimation(_) => Ok(()),
        Command::Remove(id) => remove::remove(draft, id, index),
        Command::DeclareDamage(output, _) => resources::declare_damage(draft, output),
    }
}
