//! Byte layouts of the plain-old-data geometry types.

use iceblink::geom::{
    Corners, Device, DevicePx, Logical, LogicalPx, Matrix4, Point, Rect, Size, Vec2,
};

#[test]
fn geometry_maps_onto_its_bytes_and_back() {
    let rect: Rect<LogicalPx, Logical> = Rect::new(
        Point::new(LogicalPx(1.0), LogicalPx(2.0)),
        Size::new(LogicalPx(3.0), LogicalPx(4.0)),
    );
    let bytes = bytemuck::bytes_of(&rect);
    assert_eq!(bytes.len(), 16);
    assert_eq!(
        bytemuck::pod_read_unaligned::<Rect<LogicalPx, Logical>>(bytes),
        rect
    );
}

#[test]
fn the_bytes_are_the_fields_in_declaration_order() {
    let rect: Rect<LogicalPx, Logical> = Rect::new(
        Point::new(LogicalPx(1.0), LogicalPx(2.0)),
        Size::new(LogicalPx(3.0), LogicalPx(4.0)),
    );
    let floats: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&rect));
    assert_eq!(floats, &[1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn elliptical_radii_are_eight_floats_in_corner_order() {
    let radii: Corners<Vec2<DevicePx>> = Corners::new(
        Vec2::new(DevicePx(1.0), DevicePx(2.0)),
        Vec2::new(DevicePx(3.0), DevicePx(4.0)),
        Vec2::new(DevicePx(5.0), DevicePx(6.0)),
        Vec2::new(DevicePx(7.0), DevicePx(8.0)),
    );
    let floats: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&radii));
    assert_eq!(floats, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn a_matrix_is_sixteen_floats_in_column_order() {
    let matrix = Matrix4::translation(7.0, 8.0, 9.0);
    let floats: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&matrix));
    assert_eq!(&floats[12..], &[7.0, 8.0, 9.0, 1.0]);
}

#[test]
fn a_zeroed_point_is_the_origin() {
    let point: Point<DevicePx, Device> = bytemuck::Zeroable::zeroed();
    assert_eq!(point, Point::ORIGIN);
}

#[test]
fn a_space_marker_takes_no_bytes() {
    assert_eq!(
        size_of::<Point<LogicalPx, Logical>>(),
        size_of::<[LogicalPx; 2]>()
    );
}
