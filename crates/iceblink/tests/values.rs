//! Validity of the enumeration and bit-set newtypes, and the rules values decide.

use std::sync::Arc;

use iceblink::{
    BlendMode, Color, EffectNode, Filter, Gradient, GradientShape, GradientStop, ImageFlags,
    Interpolation, MarkFlags, Paint, PaintKind,
};

#[test]
fn enumerations_accept_only_their_listed_values() {
    assert!(PaintKind::SIDES.is_valid());
    assert!(!PaintKind(3).is_valid());
    assert!(BlendMode::PLUS_LIGHTER.is_valid());
    assert!(!BlendMode(17).is_valid());
}

#[test]
fn bit_sets_accept_only_their_listed_bits() {
    let both = ImageFlags::GRAYSCALE | ImageFlags::MASK;

    assert!(both.is_valid() && both.contains(ImageFlags::MASK));
    assert!(!ImageFlags(1 << 2).is_valid());
}

#[test]
fn mark_flags_accept_listed_caps_only() {
    let flags = MarkFlags::UNION | MarkFlags::caps(MarkFlags::CAP_SQUARE, MarkFlags::CAP_ROUND);

    assert!(flags.is_valid());
    assert_eq!((flags.start_cap(), flags.end_cap()), (1, 2));
    assert!(!MarkFlags::caps(3, 0).is_valid());
    assert!(!MarkFlags(1 << 3).is_valid());
}

#[test]
fn an_effect_isolates_when_any_rule_holds() {
    let plain = EffectNode {
        clip: None,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from([]),
        isolate: false,
    };

    assert!(!plain.isolates());
    assert!(
        EffectNode {
            opacity: 0.5,
            ..plain.clone()
        }
        .isolates()
    );
    assert!(
        EffectNode {
            blend: BlendMode::MULTIPLY,
            ..plain.clone()
        }
        .isolates()
    );
    assert!(
        EffectNode {
            filters: Arc::from([Filter::Opacity(1.0)]),
            ..plain.clone()
        }
        .isolates()
    );
    assert!(
        EffectNode {
            isolate: true,
            ..plain
        }
        .isolates()
    );
}

#[test]
fn a_paint_reports_the_kind_its_references_use() {
    let gradient = Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0; 2],
            end: [1.0, 0.0],
        },
        stops: Arc::from([GradientStop::new(0.0, iceblink::color::Color::BLACK)]),
        interpolation: Interpolation::new(iceblink::ColorSpace::Oklab),
        repeat: false,
    });

    assert_eq!(Paint::Solid(Color::TRANSPARENT).kind(), PaintKind::SOLID);
    assert_eq!(gradient.kind(), PaintKind::GRADIENT);
    assert_eq!(
        Paint::Sides([Color::TRANSPARENT; 4]).kind(),
        PaintKind::SIDES
    );
}

#[test]
fn a_straight_colour_is_premultiplied() {
    assert_eq!(
        Color::from_straight(1.0, 0.5, 0.0, 0.5),
        Color([0.5, 0.25, 0.0, 0.5])
    );
    assert_eq!(
        Color::from(iceblink::color::Color::srgb(1.0, 0.0, 0.0, 0.5)),
        Color([0.5, 0.0, 0.0, 0.5])
    );
}
