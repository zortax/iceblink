//! Resources: image writes inside their image, and shaders with listed values.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    ImageDesc, ImageFormat, ImageWrite, RejectReason, ShaderDesc, ShaderMode, ShaderReads,
    ShaderSource,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::cases::rejections::rejects;
use crate::conformance::scene::stage::Stage;

/// Image writes lie inside the image and match its format, and images and writes of zero size are
/// accepted.
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

/// A shader's mode and reads are listed values and its parameters fit in 64 bytes.
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
}
