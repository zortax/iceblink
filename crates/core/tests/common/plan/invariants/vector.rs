//! The vector plan's passes and regions.

use std::collections::HashMap;

use iceblink::damage::DamageSet;
use iceblink::{DeviceRect, Frame};
use iceblink_core::plan::pass::region::TILE;

use crate::common::plan::scene::{Pushed, Scene};

use super::output;

/// Checks the vector plan:
///
/// 1. The passes partition the items in order.
/// 2. A region lies on the output and on the tile grid, or ends at the output's edge.
/// 3. A pass lists its paths in paint order, and its clip and each item's residual make up the
///    path's clip chain.
/// 4. A region holds the damaged ink of each of its paths.
/// 5. The paths of an instanced pass are disjoint.
pub(super) fn check_vector_plan(scene: &Scene, damage: &DamageSet, frame: &Frame) {
    let plan = &frame.vector_plan;
    let mut clips: HashMap<u32, (u32, DeviceRect)> = HashMap::new();
    for logged in &scene.log {
        if let Pushed::Path { index, clip } = logged.pushed {
            clips.insert(index, (clip, logged.ink));
        }
    }

    let output = output(scene);
    let mut next = 0;
    for pass in &plan.passes {
        assert_eq!(
            pass.items.start, next,
            "the passes partition the items in order"
        );
        next = pass.items.end;
    }
    assert_eq!(
        next as usize,
        plan.items.len(),
        "every item belongs to a pass"
    );

    for pass in &plan.passes {
        let region = pass.region;
        assert!(output.contains_rect(region), "a region lies on the output");
        for edge in [region.left(), region.top()] {
            assert_eq!(edge % TILE, 0, "a region starts on the tile grid");
        }
        assert!(
            region.right() % TILE == 0 || region.right() == output.right(),
            "a region ends on the tile grid or the output's edge"
        );
        assert!(
            region.bottom() % TILE == 0 || region.bottom() == output.bottom(),
            "a region ends on the tile grid or the output's edge"
        );
        let items = plan.items_of(pass);
        assert!(!items.is_empty(), "a pass holds a path");
        assert!(
            items.windows(2).all(|pair| pair[0].path < pair[1].path),
            "a pass lists its paths in paint order"
        );
        for item in items {
            let (clip, ink) = clips[&item.path];
            assert_eq!(item.clip, clip);
            let mut chain = scene.clips.links(pass.clip);
            chain.extend(scene.clips.links(item.residual));
            assert_eq!(
                chain,
                scene.clips.links(clip),
                "pass clip and residual admit the chain"
            );
            let pieces: Vec<DeviceRect> = if damage.is_full() {
                vec![output]
            } else {
                damage.rects().to_vec()
            };
            for piece in pieces {
                if let Some(needed) = ink
                    .intersection(piece)
                    .and_then(|part| part.intersection(output))
                {
                    assert!(
                        region.contains_rect(needed),
                        "the region holds the damaged ink"
                    );
                }
            }
        }
        if pass.instanced {
            for (at, left) in items.iter().enumerate() {
                for right in &items[at + 1..] {
                    assert!(
                        !left.ink.intersects(right.ink),
                        "an instanced pass is disjoint"
                    );
                }
            }
        }
    }
}
