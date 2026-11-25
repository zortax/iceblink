//! An isolating group and a clip applied by an effect.

use std::sync::Arc;

use iceblink::{BlendMode, ClipNode, ClipShape, EffectNode, Filter, OrderEntry, OrderNode, ROUND};

use crate::common::stories::{background, key};
use crate::common::story::Story;

/// An isolating group with opacity and a blur, and a clip applied by a non-isolating effect, then
/// the group's opacity and blur changed.
pub(crate) const GROUPS: Story = Story {
    name: "groups",
    steps: &[
        background,
        |tx, world| {
            let orange = world.solid(tx, [0.9, 0.5, 0.1, 1.0]);
            let teal = world.solid(tx, [0.1, 0.6, 0.6, 1.0]);
            let faded = tx.create_effect(EffectNode {
                clip: None,
                opacity: 0.5,
                blend: BlendMode::NORMAL,
                filters: Arc::from(vec![Filter::Blur(2.0)]),
                isolate: true,
            });
            world.effects.push(faded);
            let group = tx.create_order(OrderNode {
                effect: Some(faded),
            });
            world.orders.push(group);
            world.boxes(
                tx,
                key(2),
                orange,
                &[[10.0, 10.0, 40.0, 40.0], [30.0, 30.0, 40.0, 40.0]],
                false,
            );
            tx.splice(group, 0, 0, &[OrderEntry::Chunk(key(2))]);
            world.append(tx, OrderEntry::Node(group));

            let cut = tx.create_clip(ClipNode {
                parent: None,
                space: world.space,
                shape: ClipShape::RoundedRect {
                    rect: [70.0, 70.0, 40.0, 40.0],
                    radii: [8.0; 8],
                    shapes: ROUND,
                },
            });
            world.clips.push(cut);
            let clipping = tx.create_effect(EffectNode {
                clip: Some(cut),
                opacity: 1.0,
                blend: BlendMode::NORMAL,
                filters: Arc::from(Vec::new()),
                isolate: false,
            });
            world.effects.push(clipping);
            let clipped = tx.create_order(OrderNode {
                effect: Some(clipping),
            });
            world.orders.push(clipped);
            world.boxes(tx, key(3), teal, &[[60.0, 60.0, 60.0, 60.0]], false);
            tx.splice(clipped, 0, 0, &[OrderEntry::Chunk(key(3))]);
            world.append(tx, OrderEntry::Node(clipped));
        },
        |tx, world| {
            tx.set_effect(
                world.effects[0],
                EffectNode {
                    clip: None,
                    opacity: 0.8,
                    blend: BlendMode::NORMAL,
                    filters: Arc::from(vec![Filter::Blur(1.0)]),
                    isolate: true,
                },
            );
        },
    ],
};
