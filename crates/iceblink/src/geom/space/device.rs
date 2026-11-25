//! The device pixel space.

crate::geom::space::space!(
    Device,
    "device",
    "The output surface's pixel grid, measured in [`DevicePx`](crate::geom::DevicePx).",
    "",
    "The origin is the top-left corner of the output, x grows to the right and y grows downward.",
    "Integer coordinates lie on pixel boundaries, and the functions in",
    "[`snap`](crate::geom::snap) move geometry onto that grid.",
);
