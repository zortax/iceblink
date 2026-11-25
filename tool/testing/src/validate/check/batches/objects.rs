//! The externals, backdrops and vector passes batches draw one at a time.

use iceblink::BatchKind;

use crate::validate::Rule;
use crate::validate::check::Inspect;

/// Checks that no external or backdrop is drawn by several batches, and that every vector pass
/// is drawn by exactly one.
pub(super) fn check(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let mut externals = vec![0usize; frame.externals.len()];
    let mut backdrops = vec![0usize; frame.backdrops.len()];
    let mut passes = vec![0usize; frame.vector_plan.passes.len()];

    for (index, batch) in frame.batches.iter().enumerate() {
        let (name, uses, target) = match batch.kind {
            BatchKind::External(external) => ("external", &mut externals, external),
            BatchKind::Backdrop(backdrop) => ("backdrop", &mut backdrops, backdrop),
            BatchKind::Paths { pass } => ("vector pass", &mut passes, pass),
            _ => continue,
        };
        match uses.get_mut(target as usize) {
            Some(count) => *count += 1,
            None => inspect.push(
                Rule::BatchObjects,
                format!("batch {index} draws {name} {target} of {}", uses.len()),
            ),
        }
    }

    for (name, uses, at_least) in [
        ("external", externals, 0),
        ("backdrop", backdrops, 0),
        ("vector pass", passes, 1),
    ] {
        for (target, count) in uses.iter().enumerate() {
            if *count > 1 || *count < at_least {
                inspect.push(
                    Rule::BatchObjects,
                    format!("{name} {target} is drawn by {count} batches"),
                );
            }
        }
    }
}
