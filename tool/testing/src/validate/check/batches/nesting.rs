//! Group markers.

use iceblink::BatchKind;

use crate::validate::Rule;
use crate::validate::check::Inspect;

/// Checks that every group is drawn with one begin and one end marker or not at all and that
/// groups nest, and returns how many groups are open at each batch.
pub(super) fn check(inspect: &mut Inspect<'_>) -> Vec<usize> {
    let frame = inspect.frame;
    let mut opened = vec![0usize; frame.groups.len()];
    let mut closed = vec![0usize; frame.groups.len()];
    let mut open: Vec<u32> = Vec::new();
    let mut depths = Vec::with_capacity(frame.batches.len());

    for (index, batch) in frame.batches.iter().enumerate() {
        depths.push(open.len());
        match batch.kind {
            BatchKind::GroupBegin(group) => {
                let Some(count) = opened.get_mut(group as usize) else {
                    missing(inspect, index, group);
                    continue;
                };
                *count += 1;
                open.push(group);
            }
            BatchKind::GroupEnd(group) => {
                let Some(count) = closed.get_mut(group as usize) else {
                    missing(inspect, index, group);
                    continue;
                };
                *count += 1;
                match open.pop() {
                    Some(top) if top == group => {}
                    Some(top) => inspect.push(
                        Rule::GroupNesting,
                        format!("batch {index} closes group {group} while group {top} is open"),
                    ),
                    None => inspect.push(
                        Rule::GroupNesting,
                        format!("batch {index} closes group {group}, which is not open"),
                    ),
                }
            }
            _ => {}
        }
    }

    if let Some(group) = open.last() {
        inspect.push(
            Rule::GroupNesting,
            format!("group {group} is still open after the last batch"),
        );
    }
    for (group, (begins, ends)) in opened.iter().zip(&closed).enumerate() {
        if begins != ends || *begins > 1 {
            inspect.push(
                Rule::GroupNesting,
                format!("group {group} has {begins} begin and {ends} end markers"),
            );
        }
    }
    depths
}

fn missing(inspect: &mut Inspect<'_>, index: usize, group: u32) {
    inspect.push(
        Rule::BatchObjects,
        format!(
            "batch {index} marks group {group} of {}",
            inspect.frame.groups.len()
        ),
    );
}
