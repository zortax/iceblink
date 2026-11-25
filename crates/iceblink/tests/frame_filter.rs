//! Frame filters read as far as the client filters they stand for.

use iceblink::{Color, Filter, FrameFilter, read_extent};

#[test]
fn a_frame_filter_reads_as_far_as_its_client_filter() {
    let shadow = FrameFilter::DropShadow {
        offset: [4.0, -2.0],
        std_dev: 3.0,
        color: Color::TRANSPARENT,
    };
    let client = Filter::DropShadow {
        offset: [4.0, -2.0],
        std_dev: 3.0,
        color: Color::TRANSPARENT,
    };
    let bounds = [10.0, 10.0, 20.0, 20.0];
    let [left, top, right, bottom] = shadow.support();

    assert_eq!(
        read_extent(bounds, &[client]),
        [
            10.0 - left,
            10.0 - top,
            20.0 + left + right,
            20.0 + top + bottom
        ]
    );
    assert_eq!(FrameFilter::Blur(2.0).support(), [8.0; 4]);
    assert_eq!(
        FrameFilter::Shader {
            shader: 0,
            params: 0,
            reach: 5.0
        }
        .support(),
        [5.0; 4]
    );
}
