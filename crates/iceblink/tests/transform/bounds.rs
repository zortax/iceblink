//! Bounds of transformed rectangles.

use iceblink::geom::{
    Device, DevicePx, Matrix4, Point, Rect, Size, transformed_bounds, translated_bounds,
};

#[test]
fn a_translation_gives_the_rectangle_its_corners_give() {
    let rect: Rect<DevicePx, Device> = Rect::new(
        Point::new(DevicePx(-3.25), DevicePx(17.1)),
        Size::new(DevicePx(120.7), DevicePx(0.3)),
    );
    for (x, y) in [(0.0, -1234.56), (0.1, 0.2), (-4096.0, -4096.0), (7.5, 3e6)] {
        let matrix = Matrix4::translation(x, y, 0.0);
        assert!(matrix.plane_translation().is_some());
        let corners = [
            matrix.transform_point(rect.left().0, rect.top().0, 0.0),
            matrix.transform_point(rect.right().0, rect.bottom().0, 0.0),
        ];
        let expected = Rect::from_corners(
            Point::new(DevicePx(corners[0][0]), DevicePx(corners[0][1])),
            Point::new(DevicePx(corners[1][0]), DevicePx(corners[1][1])),
        );
        assert_eq!(transformed_bounds(&matrix, rect), expected);
    }
    assert!(Matrix4::scale(2.0, 1.0, 1.0).plane_translation().is_none());
    let mut tilted = Matrix4::IDENTITY;
    tilted.columns[0][3] = 0.01;
    assert!(tilted.plane_translation().is_none());
}

#[test]
fn a_translation_gives_the_rectangle_the_ordered_edges_give() {
    let reference = |x: f32, y: f32, rect: Rect<DevicePx, Device>| {
        let (left, right) = (rect.left().0 + x, rect.right().0 + x);
        let (top, bottom) = (rect.top().0 + y, rect.bottom().0 + y);
        Rect::from_corners(
            Point::new(DevicePx(left.min(right)), DevicePx(top.min(bottom))),
            Point::new(DevicePx(left.max(right)), DevicePx(top.max(bottom))),
        )
    };
    let bits = |rect: Rect<DevicePx, Device>| {
        [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        ]
        .map(|v| v.0.to_bits())
    };
    let values = [-7.5, -0.0, 0.0, 0.1, 0.3, 22_040.0, f32::INFINITY, f32::NAN];
    for &origin in &values {
        for &extent in &values {
            for &by in &[-22_000.0, -0.0, 0.0, 0.1, f32::NAN] {
                let rect = Rect::new(
                    Point::new(DevicePx(origin), DevicePx(extent)),
                    Size::new(DevicePx(extent), DevicePx(origin)),
                );
                assert_eq!(
                    bits(translated_bounds(by, -by, rect)),
                    bits(reference(by, -by, rect)),
                    "{rect:?} by {by}"
                );
            }
        }
    }
}
