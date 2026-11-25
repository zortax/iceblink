//! The paint kinds and shader modes of a chunk's items, checked before the rest of the chunk.

use iceblink::{Filter, PaintKind, PaintRef, RejectReason, ShaderMode};

use crate::naive::commit::check::items::scope::Scope;

/// Fails with `PaintKindChanged` when a paint reference refers to a live paint of another kind,
/// and otherwise with `InvalidShader` when a referenced live shader is in the wrong mode.
pub(super) fn check(scope: &Scope<'_>) -> Result<(), RejectReason> {
    let chunk = scope.chunk;
    let mut paints = Vec::new();
    paints.extend(chunk.quads.iter().flat_map(|item| [item.fill, item.stroke]));
    paints.extend(
        chunk
            .shader_quads
            .iter()
            .flat_map(|item| [item.fill, item.stroke]),
    );
    paints.extend(chunk.glyph_runs.iter().map(|item| item.paint));
    paints.extend(chunk.images.iter().map(|item| item.paint));
    paints.extend(chunk.marks.iter().map(|item| item.paint));
    for item in &chunk.paths {
        paints.extend(item.fill.iter().map(|fill| fill.paint));
        paints.extend(item.stroke.iter().map(|stroke| stroke.paint));
    }

    if paints.iter().any(|paint| wrong_kind(scope, *paint)) {
        return Err(RejectReason::PaintKindChanged);
    }

    let quad_shaders =
        chunk.shader_quads.iter().any(|item| {
            scope.client.shaders.at(item.shader).is_some_and(|desc| {
                desc.mode != ShaderMode::PAINT && desc.mode != ShaderMode::COVERAGE
            })
        });
    let filter_shaders = chunk.filters.iter().flat_map(|chain| chain.iter()).any(|filter| {
        matches!(filter, Filter::Shader { shader, .. }
            if scope.client.shaders.get(*shader).is_some_and(|desc| desc.mode != ShaderMode::FILTER))
    });
    if quad_shaders || filter_shaders {
        return Err(RejectReason::InvalidShader);
    }

    Ok(())
}

/// Reports whether `paint` refers to a live paint of a kind other than its own.
fn wrong_kind(scope: &Scope<'_>, paint: PaintRef) -> bool {
    if !paint.kind.is_valid() || paint.kind == PaintKind::NONE || paint.kind == PaintKind::INLINE {
        return false;
    }

    scope
        .client
        .paints
        .at(paint.index)
        .is_some_and(|held| held.kind() != paint.kind)
}
