//! The logical coordinate space.

crate::geom::space::space!(
    Logical,
    "logical",
    "A resolution-independent space measured in [`LogicalPx`](crate::geom::LogicalPx).",
    "",
    "The origin is at the top left, x grows to the right and y grows downward. The same content",
    "has the same logical coordinates on every output, whatever its pixel density.",
    "",
    "A [`Scale<Logical, Device>`](crate::geom::Scale) converts logical geometry to",
    "[`Device`](crate::geom::Device).",
);
