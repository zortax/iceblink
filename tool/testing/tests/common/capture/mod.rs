//! Frames the transcript and capture tests are written over.

mod lanes;

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::smallvec::smallvec;
use iceblink::{
    Batch, BatchKind, BorderStyle, ClipEntry, ClipLinkEntry, ClipRef, Color, FRAME_NO_CLIP, Frame,
    FrameGradient, FrameId, FramePaint, FrameStop, GradientShape, Matrix4, PaintKind, PaintRef,
    ParamsBlock, Quad, ROUND, SpaceRef, StopEncoding,
};

use crate::common::frames::rect;

/// The side of the test output.
pub(crate) const SIDE: u32 = 128;

/// A reference to paint slot `index` of `kind`.
pub(crate) fn paint(kind: PaintKind, index: u32) -> PaintRef {
    PaintRef {
        kind,
        _pad: [0; 3],
        index,
    }
}

/// A borderless quad over `bounds` in space slot 0, filled with the solid in paint slot 0.
pub(crate) fn quad(bounds: [f32; 4]) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: paint(PaintKind::SOLID, 0),
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// A frame of the test output with full damage, replaced tables and nothing drawn.
///
/// Space slot 0 is the identity and slot 1 moves by `(4, 2)`. Paint slot 0 is opaque blue, slot 1
/// a linear gradient and slot 2 four side colours. Clip slot 0 is a rounded rectangle measured in
/// space slot 1.
pub(crate) fn base() -> Frame {
    let mut frame = Frame {
        id: FrameId(1),
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
        ..Frame::default()
    };
    frame.damage.draw = DamageSet::full();
    frame.damage.present = DamageSet::full();

    let tables = &mut frame.tables;
    tables.spaces.replace = true;
    tables.spaces.all = vec![Matrix4::IDENTITY, Matrix4::translation(4.0, 2.0, 0.0)];
    tables.paints.replace = true;
    tables.paints.all = vec![
        FramePaint::Solid(Color([0.2, 0.4, 0.6, 1.0])),
        FramePaint::Gradient(FrameGradient {
            shape: GradientShape::Linear {
                start: [0.0, 0.0],
                end: [128.0, 0.0],
            },
            encoding: StopEncoding::OKLAB,
            repeat: false,
            stops: Arc::from(vec![
                FrameStop {
                    color: [0.6, 0.2, 0.1, 1.0],
                    offset: 0.0,
                },
                FrameStop {
                    color: [0.7, -0.1, 0.05, 1.0],
                    offset: 1.0,
                },
            ]),
        }),
        FramePaint::Sides([Color([0.5, 0.5, 0.5, 1.0]); 4]),
    ];
    tables.clips.replace = true;
    tables.clips.all = vec![ClipEntry {
        links: smallvec![ClipLinkEntry {
            space: 1,
            shape: iceblink::FrameClipShape::RoundedRect {
                rect: [0.0, 0.0, 100.0, 100.0],
                radii: [4.0; 8],
                shapes: ROUND,
            },
        }],
    }];
    tables.params.replace = true;
    tables.params.all = vec![ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    }];
    frame
}

/// Appends a batch of `kind` over the whole output.
pub(crate) fn batch(frame: &mut Frame, kind: BatchKind, range: core::ops::Range<u32>) {
    frame.batches.push(Batch {
        kind,
        range,
        ink: rect(0, 0, SIDE as i32, SIDE as i32),
    });
}

/// A frame with at least one instance of every lane, every plan and every kind of work.
pub(crate) fn kitchen_sink() -> Frame {
    let mut frame = base();
    lanes::work(&mut frame);
    lanes::quads(&mut frame);
    lanes::shadow(&mut frame);
    lanes::shader_quad(&mut frame);
    lanes::line(&mut frame);
    lanes::sprites(&mut frame);
    lanes::marks(&mut frame);
    lanes::objects(&mut frame);
    lanes::batches(&mut frame);
    frame
}
