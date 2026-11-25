//! The frame slots and values of every resolved instance.

mod boxes;
mod marks;
mod sprites;

use crate::validate::check::Inspect;
use crate::validate::check::resolve::{Lanes, Sources};

/// Checks every resolved instance of every lane.
pub(crate) fn check(inspect: &mut Inspect<'_>, lanes: &Lanes<'_>) {
    each(inspect, &lanes.quads, boxes::quad);
    each(inspect, &lanes.shader_quads, boxes::shader_quad);
    each(inspect, &lanes.shadows, boxes::shadow);
    each(inspect, &lanes.lines, boxes::line);
    each(inspect, &lanes.mono, sprites::mono);
    each(inspect, &lanes.subpixel, sprites::subpixel);
    each(inspect, &lanes.color, sprites::color);
    marks::check(inspect, &lanes.marks);
}

fn each<T: Copy>(
    inspect: &mut Inspect<'_>,
    sources: &Sources<'_, T>,
    check: fn(&mut Inspect<'_>, usize, &T),
) {
    for (position, resolved) in sources.iter().enumerate() {
        if let Some(resolved) = resolved {
            check(inspect, position, &resolved.instance);
        }
    }
}
