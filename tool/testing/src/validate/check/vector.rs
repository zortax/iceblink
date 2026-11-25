//! The vector plan.

use iceblink::{DeviceRect, PlannedPass};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::rects::{output, show, within};

/// The tile size pass regions are aligned to.
const TILE: i32 = 16;

/// Checks the pass ranges, the paths they plan, and every pass region.
pub(crate) fn check(inspect: &mut Inspect<'_>) {
    if !ranges(inspect) {
        return;
    }

    planned(inspect);
}

/// Checks that the passes plan the items in order and plan every one, and checks each pass.
///
/// Returns `false` when a pass range is broken, after reporting it.
fn ranges(inspect: &mut Inspect<'_>) -> bool {
    let frame = inspect.frame;
    let plan = &frame.vector_plan;

    let mut cursor = 0u32;
    for (index, pass) in plan.passes.iter().enumerate() {
        let (start, end) = (pass.items.start, pass.items.end);
        if start != cursor || start > end || end as usize > plan.items.len() {
            inspect.push(
                Rule::VectorPlan,
                format!(
                    "pass {index} plans items {start}..{end}, where the plan continues at {cursor} \
                     of {}",
                    plan.items.len()
                ),
            );
            return false;
        }
        cursor = end;
        pass_rules(inspect, index, pass);
    }

    if cursor as usize != plan.items.len() {
        inspect.push(
            Rule::VectorPlan,
            format!(
                "planned paths {cursor}..{} belong to no pass",
                plan.items.len()
            ),
        );
    }
    true
}

/// Checks the clip slots of every planned path, and that no path is planned twice.
fn planned(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let mut planned = vec![0usize; frame.paths.len()];
    for (index, item) in frame.vector_plan.items.iter().enumerate() {
        let at = || format!("planned path {index}");
        inspect.clip(item.residual, at);
        inspect.clip(item.clip, at);
        match planned.get_mut(item.path as usize) {
            Some(count) => *count += 1,
            None => inspect.push(
                Rule::VectorPlan,
                format!("{} names path {} of {}", at(), item.path, frame.paths.len()),
            ),
        }
    }

    for (path, count) in planned.iter().enumerate() {
        if *count > 1 {
            inspect.push(
                Rule::VectorPlan,
                format!("path {path} is planned {count} times"),
            );
        }
    }
}

/// Checks the clip, region and inks of pass `index`.
fn pass_rules(inspect: &mut Inspect<'_>, index: usize, pass: &PlannedPass) {
    inspect.clip(pass.clip, || format!("pass {index}"));
    region(inspect, index, pass);
    if pass.instanced {
        disjoint(inspect, index, pass);
    }
    damaged_ink(inspect, index, pass);
}

/// Checks that the region of pass `index` lies in the output and on the tile grid.
fn region(inspect: &mut Inspect<'_>, index: usize, pass: &PlannedPass) {
    let output = output(inspect.frame);
    let region = pass.region;
    if !within(region, output) {
        inspect.push(
            Rule::VectorPlan,
            format!("pass {index} region {} leaves the output", show(region)),
        );
    }

    let aligned =
        |start: i32, end: i32, limit: i32| start % TILE == 0 && (end % TILE == 0 || end == limit);
    let on_grid = aligned(region.left(), region.right(), output.right())
        && aligned(region.top(), region.bottom(), output.bottom());
    if !region.is_empty() && !on_grid {
        inspect.push(
            Rule::VectorPlan,
            format!("pass {index} region {} is off the tile grid", show(region)),
        );
    }
}

/// Checks that the inks of instanced pass `index` do not overlap.
fn disjoint(inspect: &mut Inspect<'_>, index: usize, pass: &PlannedPass) {
    let frame = inspect.frame;
    let items = frame.vector_plan.items_of(pass);
    for (first, a) in items.iter().enumerate() {
        if let Some(second) = items[first + 1..]
            .iter()
            .position(|b| a.ink.intersects(b.ink))
        {
            inspect.push(
                Rule::VectorPlan,
                format!(
                    "instanced pass {index} has paths {first} and {} with overlapping inks",
                    first + 1 + second
                ),
            );
            break;
        }
    }
}

/// Checks that the region of pass `index` holds the damaged ink of every path it plans.
fn damaged_ink(inspect: &mut Inspect<'_>, index: usize, pass: &PlannedPass) {
    let frame = inspect.frame;
    let output = output(frame);
    let region = pass.region;
    for item in frame.vector_plan.items_of(pass) {
        let Some(path) = frame.paths.get(item.path as usize) else {
            continue;
        };
        let Some(visible) = path.ink.intersection(output) else {
            continue;
        };

        let damaged: Vec<DeviceRect> = if frame.damage.draw.is_full() {
            vec![visible]
        } else {
            frame
                .damage
                .draw
                .rects()
                .iter()
                .filter_map(|rect| rect.intersection(visible))
                .collect()
        };
        if let Some(outside) = damaged.into_iter().find(|rect| !within(*rect, region)) {
            inspect.push(
                Rule::VectorPlan,
                format!(
                    "pass {index} region {} misses damaged ink {} of path {}",
                    show(region),
                    show(outside),
                    item.path
                ),
            );
        }
    }
}
