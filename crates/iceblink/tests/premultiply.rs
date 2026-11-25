//! Premultiplication into sRGB and into linear light.

use iceblink::color::{Color, ColorSpace};

#[test]
fn premultiplication_scales_the_channels_by_alpha() {
    let color = Color::srgb(0.8, 0.4, 0.2, 0.25);
    let [red, green, blue, alpha] = color.to_premultiplied_srgb();
    assert!((red - 0.2).abs() < 1e-6);
    assert!((green - 0.1).abs() < 1e-6);
    assert!((blue - 0.05).abs() < 1e-6);
    assert_eq!(alpha, 0.25);
}

#[test]
fn out_of_gamut_channels_are_brought_into_range() {
    let wide = Color::new(ColorSpace::DisplayP3, [0.0, 1.0, 0.0], 1.0);
    for channel in wide.to_premultiplied_srgb() {
        assert!((0.0..=1.0).contains(&channel), "channel is {channel}");
    }
}

#[test]
fn a_channel_that_is_not_a_number_becomes_zero() {
    let broken = Color::srgb(f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1.0);
    assert_eq!(broken.to_premultiplied_srgb(), [0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn every_space_reaches_the_same_premultiplied_value() {
    // The same colour written in any of the fourteen spaces converts to the same four floats.
    let color = Color::srgb(0.6, 0.3, 0.1, 0.75);
    let expected = color.to_premultiplied_srgb();
    for space in ColorSpace::ALL {
        let round_tripped = color.to_space(space).to_premultiplied_srgb();
        for channel in 0..4 {
            assert!(
                (round_tripped[channel] - expected[channel]).abs() < 1e-4,
                "{space:?} channel {channel} is {}, expected {}",
                round_tripped[channel],
                expected[channel],
            );
        }
    }
}

#[test]
fn linear_premultiplication_round_trips_through_its_colour() {
    let color = Color::srgb(0.6, 0.3, 0.1, 0.75);
    let back = color.to_premultiplied_linear().to_color();
    let expected = color.to_space(ColorSpace::SrgbLinear);
    for channel in 0..3 {
        assert!((back.components()[channel] - expected.components()[channel]).abs() < 1e-6);
    }
    assert!((back.alpha() - 0.75).abs() < 1e-6);
}

#[test]
fn a_transparent_linear_value_keeps_its_channels() {
    let from = Color::srgb(1.0, 0.0, 0.0, 0.0).to_premultiplied_linear();
    let to = Color::srgb(0.0, 0.0, 1.0, 0.0).to_premultiplied_linear();
    let middle = from.lerp(to, 0.5).to_color();
    assert_eq!(middle.alpha(), 0.0);
    assert_eq!(middle.components(), [0.0, 0.0, 0.0]);
}

#[test]
fn a_ramp_to_transparent_keeps_its_hue() {
    let red = Color::srgb(1.0, 0.0, 0.0, 1.0).to_premultiplied_linear();
    let clear = Color::TRANSPARENT.to_premultiplied_linear();
    let middle = red.lerp(clear, 0.5).to_color();
    assert!((middle.alpha() - 0.5).abs() < 1e-6);
    assert!((middle.components()[0] - 1.0).abs() < 1e-6);
}
