//! The parts of [`kitchen_sink`](super::kitchen_sink).

use std::sync::Arc;

use iceblink::kurbo::{BezPath, Circle, Shape, Stroke};
use iceblink::{
    AtlasTile, Backdrop, BatchKind, BlendMode, BorderStyle, ChunkNote, ClientId, ClipRef, Color,
    ColorSprite, DrawnChunk, ExternalQuad, FRAME_NO_CLIP, FillRule, Frame, FrameBackdrop,
    FrameExternal, FrameFilter, FrameGroup, FrameMarkPayload, FrameMarks, FramePath, FrameShadow,
    IdSpace, ItemSource, Line, LineStyle, LoweredChunk, MarkBin, MarkFlags, MarkPayload, Marks,
    OpaqueCover, PageFormat, PageOp, PaintKind, PaintRef, PassKey, PathFill, PathStroke,
    PlannedPass, PlannedPath, ROUND, ShaderQuad, Shadow, SpaceRef, Sprite, SpriteFlags,
    TextureDesc,
};

use crate::common::capture::{batch, paint, quad};
use crate::common::frames::rect;

/// Adds the page, shader and chunk work.
pub(super) fn work(frame: &mut Frame) {
    frame.resources.pages.push(PageOp::Create {
        page: 0,
        format: PageFormat::A8,
        size: [64, 64],
        levels: 1,
    });
    frame.resources.pages.push(PageOp::Create {
        page: 1,
        format: PageFormat::RGBA8_PREMULTIPLIED,
        size: [64, 64],
        levels: 1,
    });
    frame.resources.pages.push(PageOp::Upload {
        page: 0,
        level: 0,
        rect: rect(0, 0, 4, 4),
        bytes: Arc::from(vec![0xffu8; 16]),
        offset: 0,
        stride: 4,
    });
    frame.resources.shaders.push(iceblink::ShaderOp::Create {
        shader: 0,
        desc: Arc::new(iceblink::ShaderDesc {
            mode: iceblink::ShaderMode::PAINT,
            source: iceblink::ShaderSource {
                wgsl: "fn shade() {}".into(),
                prebuilt: None,
            },
            params_size: 0,
            reads: iceblink::ShaderReads::default(),
        }),
    });
    frame.chunk_notes.push(ChunkNote::Inserted {
        revision: 9,
        chunk: Arc::new(LoweredChunk {
            quads: vec![quad([0.0, 48.0, 16.0, 16.0])],
            ..LoweredChunk::default()
        }),
    });
}

/// Adds a resident chunk drawn at an offset, then two transient quads.
pub(super) fn quads(frame: &mut Frame) {
    frame.drawn.push(DrawnChunk {
        revision: 9,
        offset: [10.0, 0.0],
    });
    let mut bordered = quad([8.0, 8.0, 48.0, 24.0]);
    bordered.border = [1.0, 2.0, 1.0, 2.0];
    bordered.stroke = paint(PaintKind::SIDES, 2);
    bordered.style = BorderStyle::DASHED;
    bordered.radii = [3.0; 8];
    let mut clipped = quad([60.0, 8.0, 20.0, 20.0]);
    clipped.fill = paint(PaintKind::GRADIENT, 1);
    clipped.clip = ClipRef(0);
    clipped.space = SpaceRef(1);
    let lane = &mut frame.lanes.quads;
    lane.sources = vec![
        ItemSource::resident(0, 0),
        ItemSource::transient(0),
        ItemSource::transient(1),
    ];
    lane.items = vec![bordered, clipped];
}

pub(super) fn shadow(frame: &mut Frame) {
    frame.lanes.shadows.items.push(FrameShadow {
        bounds: [2.0, 2.0, 60.0, 36.0],
        shadow: Shadow {
            shape_bounds: [8.0, 8.0, 48.0, 24.0],
            radii: [3.0; 8],
            caster_bounds: [8.0, 8.0, 48.0, 24.0],
            caster_radii: [3.0; 8],
            shapes: ROUND,
            color: Color([0.0, 0.0, 0.0, 0.5]),
            blur: 2.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            inset: 0,
        },
    });
    frame.lanes.shadows.sources.push(ItemSource::transient(0));
}

pub(super) fn shader_quad(frame: &mut Frame) {
    frame.lanes.shader_quads.items.push(ShaderQuad {
        bounds: [90.0, 8.0, 30.0, 30.0],
        radii: [0.0; 8],
        shapes: [4.0; 4],
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [90.0, 8.0],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        shader: 0,
        params: 0,
        opacity: 0.75,
    });
    frame
        .lanes
        .shader_quads
        .sources
        .push(ItemSource::transient(0));
}

pub(super) fn line(frame: &mut Frame) {
    frame.lanes.lines.items.push(Line {
        bounds: [8.0, 40.0, 48.0, 3.0],
        color: Color([0.8, 0.0, 0.0, 0.8]),
        thickness: 1.0,
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style: LineStyle::WAVY,
        _pad: [0; 3],
    });
    frame.lanes.lines.sources.push(ItemSource::transient(0));
}

/// Adds one mono and one colour sprite.
pub(super) fn sprites(frame: &mut Frame) {
    frame.lanes.mono.items.push(Sprite {
        bounds: [20.0, 60.0, 4.0, 4.0],
        paint: paint(PaintKind::SOLID, 0),
        paint_origin: [0.0; 2],
        tile: AtlasTile {
            page: 0,
            rect: [0, 0, 4, 4],
        },
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    });
    frame.lanes.mono.sources.push(ItemSource::transient(0));

    frame.lanes.color.items.push(ColorSprite {
        bounds: [40.0, 60.0, 16.0, 16.0],
        frame: [40.0, 60.0, 16.0, 16.0],
        radii: [0.0; 8],
        shapes: ROUND,
        tile: AtlasTile {
            page: 1,
            rect: [0, 0, 16, 16],
        },
        paint: paint(PaintKind::SOLID, 0),
        paint_origin: [0.0; 2],
        opacity: 1.0,
        clip: FRAME_NO_CLIP,
        space: 0,
        flags: SpriteFlags::PAINT_ALPHA,
    });
    frame.lanes.color.sources.push(ItemSource::transient(0));
}

/// Adds the union marks instance and its bin.
pub(super) fn marks(frame: &mut Frame) {
    frame.lanes.marks.items.push(FrameMarks {
        marks: Marks {
            bounds: [64.0, 64.0, 32.0, 32.0],
            paint: paint(PaintKind::SOLID, 0),
            paint_origin: [0.0; 2],
            origin: [64.0, 64.0],
            axes: [1.0, 0.0, 0.0, 1.0],
            half_width: 0.0,
            discs: 2,
            boxes: 0,
            vertices: 0,
            placements: 0,
            first: 0,
            payload: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            flags: MarkFlags::UNION,
        },
        page: 0,
    });
    frame.lanes.marks.sources.push(ItemSource::transient(0));
    frame.lanes.mark_payloads.push(FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            discs: vec![[8.0, 8.0, 6.0, 0.0], [20.0, 20.0, 6.0, 3.0]],
            ..MarkPayload::default()
        }),
        cells: Arc::from(Vec::new()),
    });
    frame.mark_plan.bins.push(MarkBin {
        item: 0,
        region: rect(64, 64, 32, 32),
        at: [0, 0],
        page: 0,
    });
    frame.mark_plan.pages = 1;
    frame.mark_plan.extent = [32, 32];
    frame.mark_plan.full_damage = true;
}

/// Adds the path, group, external and backdrop.
pub(super) fn objects(frame: &mut Frame) {
    let mut path = BezPath::new();
    path.move_to((70.0, 70.0));
    path.line_to((90.0, 70.0));
    path.quad_to((90.0, 90.0), (70.0, 90.0));
    path.close_path();
    frame.paths.push(FramePath {
        path: Arc::new(path),
        transform: iceblink::kurbo::Affine::IDENTITY,
        brush: iceblink::kurbo::Affine::IDENTITY,
        fill: Some(PathFill {
            paint: paint(PaintKind::GRADIENT, 1),
            rule: FillRule::EVEN_ODD,
        }),
        stroke: Some(PathStroke {
            paint: paint(PaintKind::SOLID, 0),
            style: Stroke::new(2.0).with_dashes(0.0, [4.0, 2.0]),
        }),
        clips: Arc::from(vec![iceblink::PathClip {
            path: Arc::new(Circle::new((80.0, 80.0), 12.0).to_path(1.0)),
            rule: FillRule::NON_ZERO,
        }]),
        ink: rect(68, 68, 24, 24),
    });
    frame.vector_plan.items.push(PlannedPath {
        path: 0,
        residual: 0,
        clip: FRAME_NO_CLIP,
        ink: rect(4, 4, 24, 24),
    });
    frame.vector_plan.passes.push(PlannedPass {
        items: 0..1,
        region: rect(64, 64, 32, 32),
        clip: FRAME_NO_CLIP,
        instanced: true,
        clamped: false,
        overhangs: false,
        key: PassKey::NONE,
    });
    frame.groups.push(FrameGroup {
        bounds: [60.0, 60.0, 40.0, 40.0],
        source: [60.0, 60.0, 40.0, 40.0],
        clip: FRAME_NO_CLIP,
        opacity: 0.5,
        blend: BlendMode::MULTIPLY,
        filters: Arc::from(vec![FrameFilter::Blur(2.0)]),
    });

    let ids = IdSpace::new(ClientId(1));
    frame.externals.push(FrameExternal {
        quad: ExternalQuad {
            bounds: [0.0, 100.0, 28.0, 28.0],
            texture: 0,
            opacity: 1.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        client: ClientId(1),
        texture: ids.mint::<TextureDesc>(),
    });
    frame.backdrops.push(FrameBackdrop {
        backdrop: Backdrop {
            bounds: [100.0, 100.0, 20.0, 20.0],
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds: [100.0, 100.0, 20.0, 20.0],
        source: [92.0, 92.0, 36.0, 36.0],
        filters: Arc::from(vec![FrameFilter::Blur(2.0)]),
    });
}

/// Adds the batches and the opaque cover, in paint order.
pub(super) fn batches(frame: &mut Frame) {
    for (kind, range) in [
        (BatchKind::Shadows, 0..1),
        (BatchKind::Quads, 0..3),
        (
            BatchKind::ShaderQuads {
                shader: 0,
                params: 0,
            },
            0..1,
        ),
        (BatchKind::GroupBegin(0), 0..0),
        (BatchKind::Marks, 0..1),
        (BatchKind::Paths { pass: 0 }, 0..0),
        (BatchKind::GroupEnd(0), 0..0),
        (BatchKind::Lines, 0..1),
        (BatchKind::Mono { page: 0 }, 0..1),
        (BatchKind::Color { page: 1 }, 0..1),
        (BatchKind::External(0), 0..0),
        (BatchKind::Backdrop(0), 0..0),
    ] {
        batch(frame, kind, range);
    }
    frame.opaque_covers.push(OpaqueCover {
        rect: rect(11, 49, 14, 14),
        batch: 1,
        at: 0,
    });
}
