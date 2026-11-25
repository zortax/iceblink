//! The resources, paints, clips, effects and order nodes of the scene.

use std::sync::Arc;

use iceblink::geom::{Point, Size};
use iceblink::{
    BlendMode, ClipId, ClipNode, ClipShape, Color, ColorSpace, DeviceRect, EffectNode, Filter,
    Gradient, GradientShape, GradientStop, ImageDesc, ImageFormat, ImageId, ImageWrite,
    Interpolation, MaskMode, OrderId, OrderNode, Paint, PaintId, ParamsId, ShaderId, ShaderMode,
    TextureDesc, TextureId, Transaction, TransformId,
};

use super::params;
use super::shaders::shader;

/// What chunk 1 of the scene draws with, and the order nodes it sits in.
pub(super) struct Resources {
    /// A grey 16 by 16 image.
    pub(super) image: ImageId,
    /// A rounded rectangle in the turned space, inside a luminance mask of `image`.
    pub(super) rounded: ClipId,
    pub(super) texture: TextureId,
    pub(super) paint_shader: ShaderId,
    pub(super) coverage_shader: ShaderId,
    pub(super) params: ParamsId,
    /// A repeating conic gradient.
    pub(super) gradient: PaintId,
    /// Four side colours.
    pub(super) sides: PaintId,
    /// A group clipped to `rounded`, with a blend mode, opacity and three filters.
    pub(super) group: OrderId,
    /// A node inside `group` that only clips to `rounded`.
    pub(super) clipped: OrderId,
}

/// Creates the resources, with clips measured in `root` and `turned`.
pub(super) fn create(
    transaction: &mut Transaction,
    root: TransformId,
    turned: TransformId,
) -> Resources {
    let image = transaction.create_image(ImageDesc {
        width: 16,
        height: 16,
        format: ImageFormat::RGBA8,
    });
    transaction.write_image(
        image,
        ImageWrite {
            rect: DeviceRect::new(Point::new(0, 0), Size::new(16, 16)),
            bytes: Arc::from(vec![180u8; 16 * 16 * 4]),
            offset: 0,
            stride: 64,
        },
    );
    let mask = transaction.create_clip(ClipNode {
        parent: None,
        space: root,
        shape: ClipShape::Mask {
            image,
            rect: [0.0, 0.0, 96.0, 64.0],
            source: [0.0, 0.0, 16.0, 16.0],
            mode: MaskMode::LUMINANCE,
        },
    });
    let rounded = transaction.create_clip(ClipNode {
        parent: Some(mask),
        space: turned,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 30.0, 30.0],
            radii: [4.0; 8],
            shapes: [2.0, 4.0, 1.0, 2.0],
        },
    });
    let texture = transaction.create_texture(TextureDesc {
        width: 8,
        height: 8,
        premultiplied: true,
    });

    let paint_shader = transaction.create_shader(shader(ShaderMode::PAINT));
    let coverage_shader = transaction.create_shader(shader(ShaderMode::COVERAGE));
    let filter_shader = transaction.create_shader(shader(ShaderMode::FILTER));
    let params = transaction.create_params(params());
    let gradient = transaction.create_paint(conic());
    let sides = transaction.create_paint(Paint::Sides([
        Color([1.0, 0.0, 0.0, 1.0]),
        Color([0.0, 1.0, 0.0, 1.0]),
        Color([0.0, 0.0, 1.0, 1.0]),
        Color([1.0, 1.0, 1.0, 1.0]),
    ]));

    let effect = transaction.create_effect(EffectNode {
        clip: Some(rounded),
        opacity: 0.8,
        blend: BlendMode::MULTIPLY,
        filters: Arc::new([
            Filter::Blur(2.0),
            Filter::DropShadow {
                offset: [2.0, 3.0],
                std_dev: 1.5,
                color: Color([0.0, 0.0, 0.0, 0.5]),
            },
            Filter::Shader {
                shader: filter_shader,
                params,
                reach: 3.0,
            },
        ]),
        isolate: false,
    });
    let plain = transaction.create_effect(EffectNode {
        clip: Some(rounded),
        opacity: 1.0,
        blend: Default::default(),
        filters: Arc::new([]),
        isolate: false,
    });
    let group = transaction.create_order(OrderNode {
        effect: Some(effect),
    });
    let clipped = transaction.create_order(OrderNode {
        effect: Some(plain),
    });

    Resources {
        image,
        rounded,
        texture,
        paint_shader,
        coverage_shader,
        params,
        gradient,
        sides,
        group,
        clipped,
    }
}

/// A repeating conic gradient from orange to translucent blue, interpolated in HSL.
fn conic() -> Paint {
    Paint::Gradient(Gradient {
        shape: GradientShape::Conic {
            center: [20.0, 20.0],
            angle: 0.5,
        },
        stops: Arc::new([
            GradientStop::new(0.0, iceblink::color::Color::srgb(1.0, 0.5, 0.0, 1.0)),
            GradientStop::new(1.0, iceblink::color::Color::srgb(0.0, 0.5, 1.0, 0.5)),
        ]),
        interpolation: Interpolation::new(ColorSpace::Hsl),
        repeat: true,
    })
}
