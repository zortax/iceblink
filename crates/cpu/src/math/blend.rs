//! The blend modes of W3C Compositing and Blending Level 1, on premultiplied gamma-encoded colour.

use iceblink::BlendMode;

/// Returns `source` blended onto `backdrop` with `mode`, both premultiplied.
///
/// The result depends on the mode:
///
/// - `PLUS_LIGHTER` adds the two per channel, up to one.
/// - `NORMAL`, a mode outside the listed constants, and any mode over or under a transparent colour
///   give source-over.
/// - Every other mode gives `cs (1 - αb) + cb (1 - αs) + αs αb B(Cb, Cs)`, with `B` the mode's
///   function of the straight colours.
pub(crate) fn blend(mode: BlendMode, backdrop: [f32; 4], source: [f32; 4]) -> [f32; 4] {
    let alpha_s = source[3];
    let alpha_b = backdrop[3];

    if mode == BlendMode::PLUS_LIGHTER {
        return [0, 1, 2, 3].map(|channel| (source[channel] + backdrop[channel]).min(1.0));
    }

    let alpha = alpha_s + alpha_b * (1.0 - alpha_s);
    if mode == BlendMode::NORMAL || alpha_s <= 0.0 || alpha_b <= 0.0 || !separable_or_known(mode) {
        let keep = 1.0 - alpha_s;
        return [
            source[0] + backdrop[0] * keep,
            source[1] + backdrop[1] * keep,
            source[2] + backdrop[2] * keep,
            alpha,
        ];
    }

    let cs = [
        source[0] / alpha_s,
        source[1] / alpha_s,
        source[2] / alpha_s,
    ]
    .map(unit);
    let cb = [
        backdrop[0] / alpha_b,
        backdrop[1] / alpha_b,
        backdrop[2] / alpha_b,
    ]
    .map(unit);

    let mixed = mix(mode, cb, cs);
    let mut out = [0.0; 4];
    for channel in 0..3 {
        out[channel] = source[channel] * (1.0 - alpha_b)
            + backdrop[channel] * (1.0 - alpha_s)
            + alpha_s * alpha_b * mixed[channel];
    }
    out[3] = alpha;
    out
}

fn unit(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn separable_or_known(mode: BlendMode) -> bool {
    mode.0 <= BlendMode::LUMINOSITY.0
}

// The mode's function of the straight backdrop and source colours.
fn mix(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    let each = |function: fn(f32, f32) -> f32| [0, 1, 2].map(|c| function(cb[c], cs[c]));
    match mode {
        BlendMode::MULTIPLY => each(|b, s| b * s),
        BlendMode::SCREEN => each(screen),
        BlendMode::OVERLAY => each(|b, s| hard_light(s, b)),
        BlendMode::DARKEN => each(f32::min),
        BlendMode::LIGHTEN => each(f32::max),
        BlendMode::COLOR_DODGE => each(color_dodge),
        BlendMode::COLOR_BURN => each(color_burn),
        BlendMode::HARD_LIGHT => each(hard_light),
        BlendMode::SOFT_LIGHT => each(soft_light),
        BlendMode::DIFFERENCE => each(|b, s| (b - s).abs()),
        BlendMode::EXCLUSION => each(|b, s| b + s - 2.0 * b * s),
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

fn color_dodge(b: f32, s: f32) -> f32 {
    if b == 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        (b / (1.0 - s)).min(1.0)
    }
}

fn color_burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 {
        1.0
    } else if s <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - b) / s).min(1.0)
    }
}

fn soft_light(b: f32, s: f32) -> f32 {
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
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        out = out.map(|channel| l + (channel - l) * l / (l - n));
    }
    if x > 1.0 {
        out = out.map(|channel| l + (channel - l) * (1.0 - l) / (x - l));
    }
    out
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|channel| channel + d))
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
    c.map(|channel| (channel - min) * s / (max - min))
}

#[cfg(test)]
mod tests {
    use super::blend;
    use iceblink::BlendMode;

    #[test]
    fn normal_is_source_over() {
        let out = blend(
            BlendMode::NORMAL,
            [0.0, 0.0, 1.0, 1.0],
            [0.5, 0.0, 0.0, 0.5],
        );
        assert_eq!(out, [0.5, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn multiply_of_opaque_colours_multiplies_them() {
        let out = blend(
            BlendMode::MULTIPLY,
            [0.5, 1.0, 0.2, 1.0],
            [0.5, 0.5, 1.0, 1.0],
        );
        assert_eq!(out, [0.25, 0.5, 0.2, 1.0]);
    }

    #[test]
    fn difference_of_white_inverts() {
        let out = blend(
            BlendMode::DIFFERENCE,
            [0.25, 0.5, 1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );
        assert_eq!(out, [0.75, 0.5, 0.0, 1.0]);
    }

    #[test]
    fn a_mode_over_transparent_leaves_the_source() {
        let out = blend(BlendMode::SCREEN, [0.0; 4], [0.2, 0.3, 0.4, 0.5]);
        assert_eq!(out, [0.2, 0.3, 0.4, 0.5]);
    }
}
