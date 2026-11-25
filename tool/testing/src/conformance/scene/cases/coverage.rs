//! Coverage of damage: a frame draws every item whose ink meets its draw damage.

use iceblink::Built;

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{key, pixels, record_quads};
use crate::conformance::scene::stage::Stage;

/// A grid of four by four quads, each with its own colour, overlapping their neighbours.
fn grid(changed: Option<usize>) -> Vec<([f32; 4], [u8; 4])> {
    (0..16)
        .map(|index| {
            let column = (index % 4) as f32;
            let row = (index / 4) as f32;
            let bounds = [4.0 + column * 30.0, 4.0 + row * 30.0, 34.0, 34.0];
            let shade = if changed == Some(index) { 255 } else { 0 };
            (bounds, [16 * index as u8, shade, 128, 255])
        })
        .collect()
}

/// After a change, every quad whose ink meets the draw damage is drawn again, and a precise
/// compositor draws no chunk whose ink misses it.
pub fn damage_coverage(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    for part in 0..4u32 {
        let quads: Vec<_> = grid(None)[part as usize * 4..part as usize * 4 + 4].to_vec();
        stage.add_quads(key(u64::from(part)), &quads);
    }
    stage.draw();

    let changed = grid(Some(5));
    let mut tx = stage.tx();
    record_quads(&mut tx, key(1), stage.props(), &changed[4..8]);
    stage.commit(tx);
    assert_eq!(stage.build(), Built::Frame);
    for (bounds, color) in &changed {
        let ink = pixels(*bounds);
        let meets = stage.damage().is_full()
            || stage
                .damage()
                .rects()
                .iter()
                .any(|rect| rect.intersects(ink));
        if meets {
            assert!(
                !stage.picture.quads_of(*color).is_empty(),
                "the quad at {bounds:?} meets the damage {:?} but is not drawn",
                stage.damage()
            );
        }
    }

    // The last row's chunk lies below the changed quad's ink.
    if stage.precise("damage_coverage") && !stage.damage().is_full() {
        for (bounds, color) in &changed[12..] {
            assert!(
                stage.picture.quads_of(*color).is_empty(),
                "a precise compositor drew the quad at {bounds:?}, whose chunk misses the damage \
                 {:?}",
                stage.damage()
            );
        }
    }
}
