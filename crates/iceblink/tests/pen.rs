//! The split of a glyph's position into a pen pixel and a phase.

use iceblink::{GlyphPlacement, PHASES, PenPosition};

/// Half a quantisation step, the most a position may move.
const TOLERANCE: f32 = 0.5 / PHASES as f32;

#[test]
fn the_pen_and_the_phase_describe_the_position_at_every_fraction() {
    for whole in [-3.0f32, 0.0, 7.0, 1024.0] {
        for thousandth in 0..1000 {
            let position = whole + thousandth as f32 / 1000.0;
            let split = PenPosition::of(position);

            assert_eq!(
                split.quantised(),
                split.pen() + f32::from(split.phase()) / 4.0
            );
            assert!((split.quantised() - position).abs() <= TOLERANCE + 1e-4);
            assert_eq!(split.pen().fract(), 0.0);
            assert!(split.phase() < PHASES);
        }
    }
}

#[test]
fn a_fraction_that_rounds_up_moves_the_pen() {
    let split = PenPosition::of(12.9);

    assert_eq!(split.phase(), 0);
    assert_eq!(split.pen(), 13.0);
}

#[test]
fn a_bitmap_lands_on_whole_pixels() {
    let placement = GlyphPlacement { left: 1, top: 8 };

    assert_eq!(
        PenPosition::of(10.5).bitmap_origin(20.4, placement),
        [11.0, 12.0]
    );
}
