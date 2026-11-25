//! Checks of a recorded chunk against the state its commit leaves.

mod analytic;
mod content;
mod kinds;
mod marks;
mod ops;
mod path;
mod scope;

use std::ops::Range;

use iceblink::FontRegistry;
use iceblink::{LocalParent, RecordedChunk, RejectReason};

use crate::naive::commit::check::{finite, nodes};
use crate::naive::scene::Client;

use self::scope::Scope;

/// Checks every id, slot, range and number of a recorded chunk.
///
/// # Errors
///
/// Fails with the first broken rule in this order:
///
/// 1. `StaleId` when an id of `refs`, the props, the local clips or the filter chains is not
///    live.
/// 2. `PaintKindChanged` when a paint reference refers to a paint of another kind.
/// 3. `InvalidShader` when a referenced shader is in the wrong mode.
/// 4. `InvalidItem` for every other broken rule.
pub(crate) fn recorded(
    client: &Client,
    fonts: &dyn FontRegistry,
    chunk: &RecordedChunk,
) -> Result<(), RejectReason> {
    let scope = Scope {
        client,
        chunk,
        fonts,
    };

    scope.refs()?;
    nodes::props_ids(client, &chunk.props)?;
    for clip in &chunk.local_clips {
        nodes::local_clip_ids(client, clip)?;
    }
    for chain in &chunk.filters {
        nodes::filter_ids(client, chain)?;
    }

    kinds::check(&scope)?;
    ops::check(chunk)?;

    for (index, clip) in chunk.local_clips.iter().enumerate() {
        if let LocalParent::Local(parent) = clip.parent
            && parent as usize >= index
        {
            return Err(RejectReason::InvalidItem);
        }

        nodes::shape(&clip.shape)?;
    }

    for chain in &chunk.filters {
        nodes::filters(chain)?;
    }

    if let Some(hint) = chunk.hint {
        finite::numbers(&hint)?;
    }

    items(&scope)
}

/// Checks the items of every kind, in kind order.
fn items(scope: &Scope<'_>) -> Result<(), RejectReason> {
    let chunk = scope.chunk;
    chunk
        .quads
        .iter()
        .try_for_each(|item| analytic::quad(scope, item))?;
    chunk
        .shader_quads
        .iter()
        .try_for_each(|item| analytic::shader_quad(scope, item))?;
    chunk
        .shadows
        .iter()
        .try_for_each(|item| analytic::shadow(scope, item))?;
    chunk
        .lines
        .iter()
        .try_for_each(|item| analytic::line(scope, item))?;
    chunk
        .glyph_runs
        .iter()
        .try_for_each(|item| content::glyph_run(scope, item))?;
    chunk
        .images
        .iter()
        .try_for_each(|item| content::image(scope, item))?;
    chunk
        .marks
        .iter()
        .try_for_each(|item| marks::check(scope, item))?;
    chunk
        .paths
        .iter()
        .try_for_each(|item| path::check(scope, item))?;
    chunk
        .externals
        .iter()
        .try_for_each(|item| content::external(scope, item))?;
    chunk
        .backdrops
        .iter()
        .try_for_each(|item| content::backdrop(scope, item))
}

/// Returns the indices `start..start + len`, or fails with `InvalidItem` when they overflow.
fn range(start: u32, len: u32) -> Result<Range<usize>, RejectReason> {
    let end = start.checked_add(len).ok_or(RejectReason::InvalidItem)?;
    Ok(start as usize..end as usize)
}
