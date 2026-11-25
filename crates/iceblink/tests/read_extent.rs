//! The region a filtered composite reads.

use std::sync::Arc;

use iceblink::{Color, Filter, ParamsBlock, ShaderDesc, read_extent};

const BOUNDS: [f32; 4] = [10.0, 20.0, 100.0, 50.0];

#[test]
fn per_pixel_filters_and_empty_blurs_read_only_the_bounds() {
    let filters = [
        Filter::Blur(0.0),
        Filter::Blur(-1.0),
        Filter::Blur(f32::NAN),
        Filter::Opacity(0.5),
        Filter::ColorMatrix([0.0; 20]),
    ];

    assert_eq!(read_extent(BOUNDS, &[]), BOUNDS);
    assert_eq!(read_extent(BOUNDS, &filters), BOUNDS);
}

#[test]
fn a_blur_reads_three_deviations_and_the_resampling_reach() {
    assert_eq!(
        read_extent(BOUNDS, &[Filter::Blur(2.0)]),
        [2.0, 12.0, 116.0, 66.0]
    );
}

#[test]
fn a_drop_shadow_reads_further_on_the_side_its_offset_points_to() {
    let shadow = Filter::DropShadow {
        offset: [4.0, -10.0],
        std_dev: 1.0,
        color: Color::TRANSPARENT,
    };

    // r = 5: left 1, top 15, right 9, bottom 0.
    assert_eq!(read_extent(BOUNDS, &[shadow]), [9.0, 5.0, 110.0, 65.0]);
}

#[test]
fn supports_add_along_a_chain_and_a_negative_reach_reads_nothing() {
    let ids = iceblink::IdSpace::new(iceblink::ClientId(1));
    let shader = |reach| Filter::Shader {
        shader: ids.mint::<ShaderDesc>(),
        params: ids.mint::<ParamsBlock>(),
        reach,
    };
    let chain: Arc<[Filter]> = Arc::from([shader(3.0), shader(-8.0), Filter::Blur(1.0)]);

    assert_eq!(read_extent(BOUNDS, &chain), [2.0, 12.0, 116.0, 66.0]);
}
