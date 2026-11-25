//! Which logged entries a plan draws, and in what order.

use iceblink::damage::DamageSet;
use iceblink::{BatchKind, Frame};

use crate::common::plan::scene::{Logged, Pushed, Scene};

use super::{DrawKey, output};

/// Reports whether a logged entry must be drawn.
fn must_draw(scene: &Scene, damage: &DamageSet, logged: &Logged) -> bool {
    match logged.pushed {
        Pushed::Backdrop { .. } | Pushed::GroupBegin { .. } | Pushed::GroupEnd { .. } => true,
        _ => logged
            .ink
            .intersection(output(scene))
            .is_some_and(|shown| damage.intersects(shown)),
    }
}

/// Checks that exactly the entries that meet the output and the damage are drawn.
pub(super) fn check_coverage(scene: &Scene, damage: &DamageSet, keys: &[Option<DrawKey>]) {
    for (logged, key) in scene.log.iter().zip(keys) {
        assert_eq!(
            must_draw(scene, damage, logged),
            key.is_some(),
            "{logged:?} is drawn exactly when it meets the damage"
        );
    }
}

/// Checks that overlapping entries are drawn in paint order.
///
/// Two paths in one pass count as drawn together.
pub(super) fn check_paint_order(scene: &Scene, keys: &[Option<DrawKey>]) {
    for (first, (earlier, earlier_key)) in scene.log.iter().zip(keys).enumerate() {
        let Some(earlier_key) = earlier_key else {
            continue;
        };
        if is_marker(earlier) {
            continue;
        }
        for (later, later_key) in scene.log[first + 1..].iter().zip(&keys[first + 1..]) {
            let Some(later_key) = later_key else { continue };
            if is_marker(later) || !earlier.ink.intersects(later.ink) {
                continue;
            }
            let both_paths = matches!(earlier.pushed, Pushed::Path { .. })
                && matches!(later.pushed, Pushed::Path { .. });
            if both_paths && earlier_key == later_key {
                continue;
            }
            assert!(
                earlier_key < later_key,
                "{earlier:?} at {earlier_key:?} is painted before {later:?} at {later_key:?}"
            );
        }
    }
}

fn is_marker(logged: &Logged) -> bool {
    matches!(
        logged.pushed,
        Pushed::GroupBegin { .. } | Pushed::GroupEnd { .. }
    )
}

/// Checks that group markers pair up, nest, and enclose exactly their content.
pub(super) fn check_groups(scene: &Scene, frame: &Frame, keys: &[Option<DrawKey>]) {
    let mut open = Vec::new();
    for batch in &frame.batches {
        match batch.kind {
            BatchKind::GroupBegin(index) => open.push(index),
            BatchKind::GroupEnd(index) => assert_eq!(open.pop(), Some(index), "groups nest"),
            _ => {}
        }
    }
    assert!(open.is_empty(), "every group ends");

    let mut stack = Vec::new();
    for (at, logged) in scene.log.iter().enumerate() {
        match logged.pushed {
            Pushed::GroupBegin { .. } => stack.push(at),
            Pushed::GroupEnd { .. } => {
                let begin = stack.pop().expect("groups nest in the log");
                let (low, _) = keys[begin].expect("a group is drawn");
                let (high, _) = keys[at].expect("a group is drawn");
                for (inside, key) in keys.iter().enumerate() {
                    let Some((batch, _)) = key else { continue };
                    if inside == begin || inside == at {
                        continue;
                    }
                    if inside > begin && inside < at {
                        assert!(
                            *batch > low && *batch < high,
                            "{:?} lies inside its group's markers",
                            scene.log[inside]
                        );
                    } else if inside < begin {
                        assert!(
                            *batch < low,
                            "{:?} is drawn before the group",
                            scene.log[inside]
                        );
                    } else {
                        assert!(
                            *batch > high,
                            "{:?} is drawn after the group",
                            scene.log[inside]
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
