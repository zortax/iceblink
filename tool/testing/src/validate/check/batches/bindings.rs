//! The page or shader binding every instance of a batch shares.

use core::ops::Range;

use iceblink::BatchKind;

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::resolve::{Lanes, Sources};

/// Checks that every known instance of batch `index` has the batch's page or shader binding.
pub(super) fn check(
    inspect: &mut Inspect<'_>,
    lanes: &Lanes<'_>,
    index: usize,
    kind: BatchKind,
    range: Range<usize>,
) {
    match kind {
        BatchKind::Mono { page } => {
            sprites(inspect, &lanes.mono, index, page, range, |sprite| {
                sprite.tile.page
            });
        }
        BatchKind::Subpixel { page } => {
            sprites(inspect, &lanes.subpixel, index, page, range, |sprite| {
                sprite.tile.page
            });
        }
        BatchKind::Color { page } => {
            sprites(inspect, &lanes.color, index, page, range, |sprite| {
                sprite.tile.page
            });
        }
        BatchKind::ShaderQuads { shader, params } => {
            shader_quads(inspect, lanes, index, shader, params, range);
        }
        _ => {}
    }
}

/// Checks that every known sprite of batch `index` lies on the batch's page, which `page_of`
/// reads from a sprite.
fn sprites<T>(
    inspect: &mut Inspect<'_>,
    sources: &Sources<'_, T>,
    index: usize,
    page: u32,
    range: Range<usize>,
    page_of: fn(&T) -> u32,
) {
    let stray = sources[range]
        .iter()
        .flatten()
        .map(|sprite| page_of(&sprite.instance))
        .find(|found| *found != page);
    if let Some(found) = stray {
        inspect.push(
            Rule::BatchBindings,
            format!("batch {index} binds page {page} and draws a sprite on page {found}"),
        );
    }
}

/// Checks the `shader` and `params` slots batch `index` binds, and that every known quad of the
/// batch has them.
fn shader_quads(
    inspect: &mut Inspect<'_>,
    lanes: &Lanes<'_>,
    index: usize,
    shader: u32,
    params: u32,
    range: Range<usize>,
) {
    let at = || format!("batch {index}");
    inspect.shader(shader, at);
    inspect.params(params, at);

    let stray = lanes.shader_quads[range]
        .iter()
        .flatten()
        .find(|quad| quad.instance.shader != shader || quad.instance.params != params);
    if let Some(quad) = stray {
        inspect.push(
            Rule::BatchBindings,
            format!(
                "batch {index} binds shader {shader} with parameters {params}, and draws a quad \
                 with shader {} and parameters {}",
                quad.instance.shader, quad.instance.params
            ),
        );
    }
}
