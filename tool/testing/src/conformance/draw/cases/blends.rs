//! Blend modes, each checked against the W3C formula where the renderer declares it and against
//! source-over where it does not.

use iceblink::{BlendMode, FrameGroup};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, group, opaque, straight};
use crate::conformance::draw::canvas::{Canvas, SIZE};

/// How far a blended channel may be from the formula, in eight-bit steps.
const STEPS: u8 = 3;

/// The backdrop's straight colour, opaque.
const BACKDROP: [f32; 3] = [0.8, 0.4, 0.2];

/// The source's straight colour.
const SOURCE: [f32; 3] = [0.3, 0.6, 0.9];

/// The source's alpha.
const SOURCE_ALPHA: f32 = 0.75;

/// The cell of mode `index`: six columns of 20 pixels, three rows.
fn spot(index: usize) -> [f32; 4] {
    let column = (index % 6) as f32;
    let row = (index / 6) as f32;
    [4.0 + column * 20.0, 4.0 + row * 20.0, 16.0, 16.0]
}

/// Every mode, drawn as a group of the source over the backdrop.
pub fn blend_modes(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let declared = canvas.caps().blend_modes;
    let mut builder = FrameBuilder::new(SIZE);
    let backdrop = builder.solid(opaque(
        byte(BACKDROP[0]),
        byte(BACKDROP[1]),
        byte(BACKDROP[2]),
    ));
    let source = builder.solid(straight(
        byte(SOURCE[0]),
        byte(SOURCE[1]),
        byte(SOURCE[2]),
        byte(SOURCE_ALPHA),
    ));
    for mode in 0..=16u8 {
        let bounds = spot(mode as usize);
        builder.quad(filled(bounds, backdrop));
        builder.begin_group(FrameGroup {
            blend: BlendMode(mode),
            ..group(bounds, 1.0, &[])
        });
        builder.quad(filled(bounds, source));
        builder.end_group();
    }
    let image = canvas.draw(builder.finish());

    for mode in 0..=16u8 {
        let drawn = declared & (1 << mode) != 0;
        let expected = expected(BlendMode(mode), drawn);
        let [x, y, width, height] = spot(mode as usize);
        let found = image.pixel((x + width / 2.0) as u32, (y + height / 2.0) as u32);
        for channel in 0..3 {
            let want = byte(expected[channel]);
            assert!(
                found[channel].abs_diff(want) <= STEPS,
                "blend mode {mode} ({}) draws {found:?} where {:?} is expected",
                if drawn { "declared" } else { "drawn as NORMAL" },
                expected.map(byte)
            );
        }
    }
}

/// Returns the straight colour the source composites to over the backdrop in `mode`, or in
/// source-over unless `drawn`.
fn expected(mode: BlendMode, drawn: bool) -> [f32; 3] {
    let cb = BACKDROP.map(quantised);
    let cs = SOURCE.map(quantised);
    let alpha = quantised(SOURCE_ALPHA);
    let blended = if drawn { blend(mode, cb, cs) } else { cs };
    if drawn && mode == BlendMode::PLUS_LIGHTER {
        [0, 1, 2].map(|i| (cs[i] * alpha + cb[i]).min(1.0))
    } else {
        [0, 1, 2].map(|i| cb[i] * (1.0 - alpha) + alpha * blended[i])
    }
}

fn byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The value an eight-bit channel holds.
fn quantised(value: f32) -> f32 {
    f32::from(byte(value)) / 255.0
}

/// `B(Cb, Cs)` of the W3C Compositing and Blending Level 1 mode, on straight colours.
fn blend(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    let separable = |f: fn(f32, f32) -> f32| [0, 1, 2].map(|i| f(cb[i], cs[i]));
    match mode {
        BlendMode::MULTIPLY => separable(|b, s| b * s),
        BlendMode::SCREEN => separable(screen),
        BlendMode::OVERLAY => separable(|b, s| hard_light(s, b)),
        BlendMode::DARKEN => separable(f32::min),
        BlendMode::LIGHTEN => separable(f32::max),
        BlendMode::COLOR_DODGE => separable(|b, s| {
            if b == 0.0 {
                0.0
            } else if s == 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }),
        BlendMode::COLOR_BURN => separable(|b, s| {
            if b == 1.0 {
                1.0
            } else if s == 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - b) / s).min(1.0)
            }
        }),
        BlendMode::HARD_LIGHT => separable(hard_light),
        BlendMode::SOFT_LIGHT => separable(|b, s| {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 {
                    ((16.0 * b - 12.0) * b + 4.0) * b
                } else {
                    b.sqrt()
                };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }),
        BlendMode::DIFFERENCE => separable(|b, s| (b - s).abs()),
        BlendMode::EXCLUSION => separable(|b, s| b + s - 2.0 * b * s),
        BlendMode::HUE => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::SATURATION => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::COLOR => set_lum(cs, lum(cb)),
        BlendMode::LUMINOSITY => set_lum(cb, lum(cs)),
        _ => cs,
    }
}

fn screen(b: f32, s: f32) -> f32 {
    b + s - b * s
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b * 2.0 * s
    } else {
        screen(b, 2.0 * s - 1.0)
    }
}

fn lum([r, g, b]: [f32; 3]) -> f32 {
    0.3 * r + 0.59 * g + 0.11 * b
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        out = out.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 {
        out = out.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    out
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    if max <= min {
        return [0.0; 3];
    }
    c.map(|v| (v - min) * s / (max - min))
}

#[cfg(test)]
mod tests {
    use iceblink::BlendMode;

    use super::blend;

    const CB: [f32; 3] = [0.8, 0.4, 0.2];
    const CS: [f32; 3] = [0.3, 0.6, 0.9];

    fn near(found: [f32; 3], expected: [f32; 3]) -> bool {
        found
            .iter()
            .zip(expected)
            .all(|(found, expected)| (found - expected).abs() < 1.0e-4)
    }

    #[test]
    fn separable_modes_match_hand_computed_values() {
        let cases = [
            (BlendMode::MULTIPLY, [0.24, 0.24, 0.18]),
            (BlendMode::SCREEN, [0.86, 0.76, 0.92]),
            (BlendMode::OVERLAY, [0.72, 0.48, 0.36]),
            (BlendMode::DARKEN, [0.3, 0.4, 0.2]),
            (BlendMode::LIGHTEN, [0.8, 0.6, 0.9]),
            (BlendMode::COLOR_DODGE, [1.0, 1.0, 1.0]),
            (BlendMode::COLOR_BURN, [0.333_333, 0.0, 0.111_111]),
            (BlendMode::HARD_LIGHT, [0.48, 0.52, 0.84]),
            (BlendMode::DIFFERENCE, [0.5, 0.2, 0.7]),
            (BlendMode::EXCLUSION, [0.62, 0.52, 0.74]),
        ];
        for (mode, expected) in cases {
            let found = blend(mode, CB, CS);
            assert!(near(found, expected), "{mode:?}: {found:?}");
        }
    }

    #[test]
    fn non_separable_modes_keep_luminosity_where_they_should() {
        let lum = |[r, g, b]: [f32; 3]| 0.3 * r + 0.59 * g + 0.11 * b;
        for mode in [BlendMode::HUE, BlendMode::SATURATION, BlendMode::COLOR] {
            let found = blend(mode, CB, CS);
            assert!(
                (lum(found) - lum(CB)).abs() < 1.0e-4,
                "{mode:?} keeps the backdrop's luminosity: {found:?}"
            );
        }
        let found = blend(BlendMode::LUMINOSITY, CB, CS);
        assert!((lum(found) - lum(CS)).abs() < 1.0e-4, "{found:?}");
        assert!(near(blend(BlendMode::COLOR, CS, CS), CS));
    }
}
