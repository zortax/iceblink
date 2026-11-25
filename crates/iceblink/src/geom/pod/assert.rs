//! The compile-time layout table.
//!
//! Each entry states a type's size, its alignment, and the offset and size of each field. The
//! offsets are compared with the compiler's layout, and the field sizes must add up to the type's
//! size without gaps, which means the type has no padding.

/// Asserts a type's size, alignment and field offsets, and that it has no padding.
///
/// Fields are listed in declaration order, each with its expected byte offset and its size. A
/// zero-sized field, such as a space marker, has a size of zero.
macro_rules! assert_layout {
    (
        $type:ty,
        size = $size:expr,
        align = $align:expr,
        fields = [$($field:tt @ $offset:expr, $width:expr);+ $(;)?]
        $(,)?
    ) => {
        const _: () = {
            assert!(
                ::core::mem::size_of::<$type>() == $size,
                concat!(stringify!($type), " is not the size the layout table claims"),
            );
            assert!(
                ::core::mem::align_of::<$type>() == $align,
                concat!(stringify!($type), " is not aligned the way the layout table claims"),
            );
            $(
                assert!(
                    ::core::mem::offset_of!($type, $field) == $offset,
                    concat!(
                        stringify!($type),
                        ".",
                        stringify!($field),
                        " is not at the offset the layout table claims",
                    ),
                );
            )+
            // Each field must begin where the previous one ended, and the last must end at the
            // size.
            let mut cursor: usize = 0;
            $(
                assert!(
                    $offset == cursor,
                    concat!(
                        stringify!($type),
                        " has padding before ",
                        stringify!($field),
                    ),
                );
                cursor += $width;
            )+
            assert!(
                cursor == $size,
                concat!(stringify!($type), " has trailing padding"),
            );
        };
    };
}

use crate::geom::corners::{Corners, Vec2};
use crate::geom::edges::Edges;
use crate::geom::point::Point;
use crate::geom::rect::Rect;
use crate::geom::size::Size;
use crate::geom::space::{Device, Logical};
use crate::geom::transform::{Affine2, Decomposed, Matrix4};
use crate::geom::unit::{DevicePx, LogicalPx, Scale};

assert_layout!(LogicalPx, size = 4, align = 4, fields = [0 @ 0, 4]);
assert_layout!(DevicePx, size = 4, align = 4, fields = [0 @ 0, 4]);

assert_layout!(
    Scale<Logical, Device>,
    size = 4,
    align = 4,
    fields = [factor @ 0, 4; spaces @ 4, 0],
);

assert_layout!(
    Point<LogicalPx, Logical>,
    size = 8,
    align = 4,
    fields = [x @ 0, 4; y @ 4, 4; space @ 8, 0],
);
assert_layout!(
    Point<DevicePx, Device>,
    size = 8,
    align = 4,
    fields = [x @ 0, 4; y @ 4, 4; space @ 8, 0],
);
assert_layout!(
    Point<i32, Device>,
    size = 8,
    align = 4,
    fields = [x @ 0, 4; y @ 4, 4; space @ 8, 0],
);

assert_layout!(
    Size<LogicalPx, Logical>,
    size = 8,
    align = 4,
    fields = [width @ 0, 4; height @ 4, 4; space @ 8, 0],
);
assert_layout!(
    Size<DevicePx, Device>,
    size = 8,
    align = 4,
    fields = [width @ 0, 4; height @ 4, 4; space @ 8, 0],
);

assert_layout!(
    Rect<LogicalPx, Logical>,
    size = 16,
    align = 4,
    fields = [origin @ 0, 8; size @ 8, 8],
);
assert_layout!(
    Rect<DevicePx, Device>,
    size = 16,
    align = 4,
    fields = [origin @ 0, 8; size @ 8, 8],
);
assert_layout!(
    Rect<i32, Device>,
    size = 16,
    align = 4,
    fields = [origin @ 0, 8; size @ 8, 8],
);

assert_layout!(
    Vec2<DevicePx>,
    size = 8,
    align = 4,
    fields = [x @ 0, 4; y @ 4, 4],
);

assert_layout!(
    Corners<LogicalPx>,
    size = 16,
    align = 4,
    fields = [top_left @ 0, 4; top_right @ 4, 4; bottom_right @ 8, 4; bottom_left @ 12, 4],
);
assert_layout!(
    Corners<Vec2<DevicePx>>,
    size = 32,
    align = 4,
    fields = [top_left @ 0, 8; top_right @ 8, 8; bottom_right @ 16, 8; bottom_left @ 24, 8],
);

assert_layout!(
    Edges<DevicePx>,
    size = 16,
    align = 4,
    fields = [top @ 0, 4; right @ 4, 4; bottom @ 8, 4; left @ 12, 4],
);

assert_layout!(
    Affine2,
    size = 24,
    align = 4,
    fields = [a @ 0, 4; b @ 4, 4; c @ 8, 4; d @ 12, 4; tx @ 16, 4; ty @ 20, 4],
);

assert_layout!(Matrix4, size = 64, align = 4, fields = [columns @ 0, 64]);

assert_layout!(
    Decomposed,
    size = 68,
    align = 4,
    fields = [
        translation @ 0, 12;
        scale @ 12, 12;
        skew @ 24, 12;
        perspective @ 36, 16;
        rotation @ 52, 16;
    ],
);
