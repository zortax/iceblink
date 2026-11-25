//! Resources: image writes inside their image, and shaders with listed values.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    ClipRef, EffectNode, Filter, ImageDesc, ImageFormat, ImageWrite, PaintRef, ParamsBlock, ROUND,
    RejectReason, ShaderDesc, ShaderMode, ShaderQuad, ShaderReads, ShaderSource, SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{faded, key};
use crate::conformance::scene::cases::rejections::rejects;
use crate::conformance::scene::stage::Stage;

/// Image writes lie inside the image, match its format and carry every byte they cover, and images
/// and writes of zero size are accepted.
pub fn invalid_writes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let image = tx.create_image(ImageDesc {
        width: 4,
        height: 4,
        format: ImageFormat::RGBA8,
    });
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(2, 2), Size::new(4, 4)),
            bytes: Arc::from(vec![255; 64]),
            offset: 0,
            stride: 16,
        },
    );
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidWrite,
        "a write past the image",
    );

    let mut tx = stage.tx();
    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(2, 2)),
            bytes: Arc::from(vec![255; 23]),
            offset: 0,
            stride: 16,
        },
    );
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidWrite,
        "a write short of bytes",
    );

    let mut tx = stage.tx();
    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(2, 2)),
            bytes: Arc::from(vec![255; 24]),
            offset: 0,
            stride: 16,
        },
    );
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.create_image(ImageDesc {
        width: 0,
        height: 4,
        format: ImageFormat::A8,
    });
    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(4, 0), Size::new(0, 4)),
            bytes: Arc::from(Vec::new()),
            offset: 0,
            stride: 16,
        },
    );
    stage.commit(tx);
}

/// A shader's mode and reads are listed values, its parameters fit in 64 bytes, and a filter or a
/// shader quad names a shader of its own mode.
pub fn invalid_shaders(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let cases = [
        (
            ShaderMode::PAINT,
            ShaderReads::default(),
            65,
            "parameters of 65 bytes",
        ),
        (ShaderMode(7), ShaderReads::default(), 0, "an unlisted mode"),
        (
            ShaderMode::PAINT,
            ShaderReads(1 << 6),
            0,
            "an unlisted reads bit",
        ),
    ];
    for (mode, reads, params_size, case) in cases {
        let mut tx = stage.tx();
        tx.create_shader(ShaderDesc {
            mode,
            source: ShaderSource {
                wgsl: Arc::from(""),
                prebuilt: None,
            },
            params_size,
            reads,
        });
        rejects(&mut stage, tx, RejectReason::InvalidShader, case);
    }

    let mut tx = stage.tx();
    let paint_shader = tx.create_shader(shader(ShaderMode::PAINT));
    let params = tx.create_params(ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    tx.create_effect(EffectNode {
        filters: Arc::from([Filter::Shader {
            shader: paint_shader,
            params,
            reach: 0.0,
        }]),
        ..faded(1.0)
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidShader,
        "a filter naming a paint shader",
    );

    let mut tx = stage.tx();
    let filter_shader = tx.create_shader(shader(ShaderMode::FILTER));
    let params = tx.create_params(ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    let mut chunk = tx.record(key(1), stage.props());
    let item = ShaderQuad {
        bounds: [0.0, 0.0, 8.0, 8.0],
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        shader: chunk.shader_slot(filter_shader),
        params: chunk.params_slot(params),
        opacity: 1.0,
    };
    chunk.shader_quad(item);
    rejects(
        &mut stage,
        tx,
        RejectReason::InvalidShader,
        "a shader quad naming a filter shader",
    );
}

/// A shader of `mode` with an empty source and no parameters.
fn shader(mode: ShaderMode) -> ShaderDesc {
    ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from(""),
            prebuilt: None,
        },
        params_size: 0,
        reads: ShaderReads::default(),
    }
}
