//! The instanced lanes and payload kinds the chunk store keeps arenas for.

use iceblink::{
    ColorSprite, FrameMarkPayload, FrameMarks, FrameShadow, Lane, Lanes, Line, LoweredChunk,
    MarkBox, MarkCell, MarkPlacement, Quad, ShaderQuad, Sprite,
};

/// The instanced lanes, in arena order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LaneKind {
    Quads,
    Shadows,
    Lines,
    Mono,
    Subpixel,
    Color,
    ShaderQuads,
    Marks,
}

impl LaneKind {
    pub(crate) const ALL: [Self; 8] = [
        Self::Quads,
        Self::Shadows,
        Self::Lines,
        Self::Mono,
        Self::Subpixel,
        Self::Color,
        Self::ShaderQuads,
        Self::Marks,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// Returns the size of one instance, in bytes.
    pub(crate) fn element(self) -> u32 {
        (match self {
            Self::Quads => size_of::<Quad>(),
            Self::Shadows => size_of::<FrameShadow>(),
            Self::Lines => size_of::<Line>(),
            Self::Mono | Self::Subpixel => size_of::<Sprite>(),
            Self::Color => size_of::<ColorSprite>(),
            Self::ShaderQuads => size_of::<ShaderQuad>(),
            Self::Marks => size_of::<FrameMarks>(),
        }) as u32
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Quads => "iceblink.arena.quads",
            Self::Shadows => "iceblink.arena.shadows",
            Self::Lines => "iceblink.arena.lines",
            Self::Mono => "iceblink.arena.mono",
            Self::Subpixel => "iceblink.arena.subpixel",
            Self::Color => "iceblink.arena.color",
            Self::ShaderQuads => "iceblink.arena.shader_quads",
            Self::Marks => "iceblink.arena.marks",
        }
    }
}

/// The number of instanced lanes.
pub(crate) const LANES: usize = LaneKind::ALL.len();

/// The lane of marks, whose order the mark pipelines read by position.
pub(crate) const MARKS_LANE: usize = LaneKind::Marks as usize;

/// The lane of shader quads.
pub(crate) const SHADER_QUADS_LANE: usize = LaneKind::ShaderQuads as usize;

/// The payload kinds of marks: discs, boxes, polyline vertices, placements and shape cells.
pub(crate) const PAYLOAD_LANES: usize = 5;

/// The payload kind of shape cells, which the placement draws read beside their own kind.
pub(crate) const CELLS: usize = 4;

/// Returns the size of one payload record of `kind`, in bytes.
pub(crate) fn payload_element(kind: usize) -> u32 {
    (match kind {
        0 => size_of::<[f32; 4]>(),
        1 => size_of::<MarkBox>(),
        2 => size_of::<[f32; 2]>(),
        3 => size_of::<MarkPlacement>(),
        _ => size_of::<MarkCell>(),
    }) as u32
}

pub(crate) fn payload_label(kind: usize) -> &'static str {
    match kind {
        0 => "iceblink.arena.mark_discs",
        1 => "iceblink.arena.mark_boxes",
        2 => "iceblink.arena.mark_vertices",
        3 => "iceblink.arena.mark_placements",
        _ => "iceblink.arena.mark_cells",
    }
}

/// Returns the bytes of payload kind `kind` of `payload`.
pub(crate) fn payload_slice(payload: &FrameMarkPayload, kind: usize) -> &[u8] {
    match kind {
        0 => bytemuck::cast_slice(&payload.payload.discs),
        1 => bytemuck::cast_slice(&payload.payload.boxes),
        2 => bytemuck::cast_slice(&payload.payload.vertices),
        3 => bytemuck::cast_slice(&payload.payload.placements),
        _ => bytemuck::cast_slice(&payload.cells),
    }
}

/// Returns the record counts of every payload kind of `payload`.
pub(crate) fn payload_counts(payload: &FrameMarkPayload) -> [u32; PAYLOAD_LANES] {
    core::array::from_fn(|kind| {
        (payload_slice(payload, kind).len() / payload_element(kind) as usize) as u32
    })
}

/// Returns the key of `payload`: the addresses of its two shared parts.
pub(crate) fn payload_key(payload: &FrameMarkPayload) -> (usize, usize) {
    (
        std::sync::Arc::as_ptr(&payload.payload) as usize,
        std::sync::Arc::as_ptr(&payload.cells) as *const u8 as usize,
    )
}

/// Returns the bytes of the array of `chunk` for `lane`.
pub(crate) fn chunk_bytes(chunk: &LoweredChunk, lane: LaneKind) -> &[u8] {
    match lane {
        LaneKind::Quads => bytemuck::cast_slice(&chunk.quads),
        LaneKind::Shadows => bytemuck::cast_slice(&chunk.shadows),
        LaneKind::Lines => bytemuck::cast_slice(&chunk.lines),
        LaneKind::Mono => bytemuck::cast_slice(&chunk.mono),
        LaneKind::Subpixel => bytemuck::cast_slice(&chunk.subpixel),
        LaneKind::Color => bytemuck::cast_slice(&chunk.color),
        LaneKind::ShaderQuads => bytemuck::cast_slice(&chunk.shader_quads),
        LaneKind::Marks => bytemuck::cast_slice(&chunk.marks),
    }
}

/// Returns the bytes of the transient instances of `lane`.
pub(crate) fn lane_items(lanes: &Lanes, lane: LaneKind) -> &[u8] {
    fn items<T: bytemuck::Pod>(lane: &Lane<T>) -> &[u8] {
        bytemuck::cast_slice(&lane.items)
    }
    match lane {
        LaneKind::Quads => items(&lanes.quads),
        LaneKind::Shadows => items(&lanes.shadows),
        LaneKind::Lines => items(&lanes.lines),
        LaneKind::Mono => items(&lanes.mono),
        LaneKind::Subpixel => items(&lanes.subpixel),
        LaneKind::Color => items(&lanes.color),
        LaneKind::ShaderQuads => items(&lanes.shader_quads),
        LaneKind::Marks => items(&lanes.marks),
    }
}

/// Returns the sources of `lane`, in draw order.
pub(crate) fn lane_sources(lanes: &Lanes, lane: LaneKind) -> &[iceblink::ItemSource] {
    match lane {
        LaneKind::Quads => &lanes.quads.sources,
        LaneKind::Shadows => &lanes.shadows.sources,
        LaneKind::Lines => &lanes.lines.sources,
        LaneKind::Mono => &lanes.mono.sources,
        LaneKind::Subpixel => &lanes.subpixel.sources,
        LaneKind::Color => &lanes.color.sources,
        LaneKind::ShaderQuads => &lanes.shader_quads.sources,
        LaneKind::Marks => &lanes.marks.sources,
    }
}
