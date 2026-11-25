//! Checks of the values that node, slot and chunk commands carry.
//!
//! A check that covers several rules reports the first broken one in this order:
//!
//! 1. `StaleId` when an id inside the value refers to no live object.
//! 2. `InvalidShader` when a referenced shader is in the wrong mode.
//! 3. `InvalidItem` when a number is not finite, a colour is invalid or an enumeration holds no
//!    listed value.

use iceblink::{
    Anchor, ChunkProps, ClipNode, ClipShape, EffectNode, Filter, GradientShape, LocalClip,
    LocalParent, Paint, ParamsBlock, RejectReason, ScrollNode, ShaderMode, StickyAnchor,
    ThumbAnchor, TransformNode,
};

use crate::naive::commit::check::{finite, live};
use crate::naive::scene::Client;

pub(crate) fn transform(client: &Client, node: &TransformNode) -> Result<(), RejectReason> {
    if let Some(parent) = node.parent {
        live(&client.transforms, parent)?;
    }

    if let Some(scroll) = node.scroll {
        live(&client.scrolls, scroll)?;
    }

    match node.anchor {
        Anchor::Sticky(StickyAnchor { scroll, .. }) | Anchor::Thumb(ThumbAnchor { scroll, .. }) => {
            live(&client.scrolls, scroll)?;
        }
        Anchor::Scrolls | Anchor::Fixed => {}
    }

    finite::matrix(&node.local)?;
    match node.anchor {
        Anchor::Sticky(sticky) => {
            finite::numbers(&sticky.rect)?;
            finite::numbers(&sticky.limit)?;
            // NaN leaves an edge free, so only infinities are rejected.
            if sticky.inset.iter().any(|inset| inset.is_infinite()) {
                return Err(RejectReason::InvalidItem);
            }
        }
        Anchor::Thumb(thumb) => finite::numbers(&thumb.track)?,
        Anchor::Scrolls | Anchor::Fixed => {}
    }

    Ok(())
}

pub(crate) fn scroll(client: &Client, node: &ScrollNode) -> Result<(), RejectReason> {
    live(&client.transforms, node.space)?;
    finite::numbers(&node.port)?;
    finite::numbers(&node.content)?;
    finite::numbers(&node.offset)?;
    if let Some(behaviour) = &node.behaviour {
        // An axis of a snap point may be NaN, which does not snap.
        let infinite = behaviour
            .snap
            .iter()
            .any(|point| point.offset.iter().any(|axis| axis.is_infinite()));
        if infinite {
            return Err(RejectReason::InvalidItem);
        }
        valid(behaviour.bounce.is_valid() && behaviour.chain.is_valid())?;
    }

    Ok(())
}

pub(crate) fn clip(client: &Client, node: &ClipNode) -> Result<(), RejectReason> {
    if let Some(parent) = node.parent {
        live(&client.clips, parent)?;
    }

    live(&client.transforms, node.space)?;
    shape_ids(client, &node.shape)?;
    shape(&node.shape)
}

/// Fails with `StaleId` when a mask refers to an image that is not live.
pub(crate) fn shape_ids(client: &Client, shape: &ClipShape) -> Result<(), RejectReason> {
    match shape {
        ClipShape::RoundedRect { .. } => Ok(()),
        ClipShape::Mask { image, .. } => live(&client.images, *image),
    }
}

pub(crate) fn shape(shape: &ClipShape) -> Result<(), RejectReason> {
    match shape {
        ClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } => {
            finite::numbers(rect)?;
            finite::numbers(radii)?;
            finite::numbers(shapes)
        }
        ClipShape::Mask {
            rect, source, mode, ..
        } => {
            finite::numbers(rect)?;
            finite::numbers(source)?;
            valid(mode.is_valid())
        }
    }
}

pub(crate) fn effect(client: &Client, node: &EffectNode) -> Result<(), RejectReason> {
    if let Some(clip) = node.clip {
        live(&client.clips, clip)?;
    }

    filter_ids(client, &node.filters)?;
    filter_modes(client, &node.filters)?;
    finite::numbers(&[node.opacity])?;
    valid(node.blend.is_valid())?;
    filters(&node.filters)
}

/// Fails with `StaleId` when a shader filter refers to a shader or parameter block that is not
/// live.
pub(crate) fn filter_ids(client: &Client, chain: &[Filter]) -> Result<(), RejectReason> {
    for filter in chain {
        if let Filter::Shader { shader, params, .. } = *filter {
            live(&client.shaders, shader)?;
            live(&client.params, params)?;
        }
    }

    Ok(())
}

/// Fails with `InvalidShader` when a shader filter refers to a live shader that is not in filter
/// mode.
pub(crate) fn filter_modes(client: &Client, chain: &[Filter]) -> Result<(), RejectReason> {
    for filter in chain {
        if let Filter::Shader { shader, .. } = *filter
            && client
                .shaders
                .get(shader)
                .is_some_and(|desc| desc.mode != ShaderMode::FILTER)
        {
            return Err(RejectReason::InvalidShader);
        }
    }

    Ok(())
}

/// Checks the numbers and colours of a filter chain.
pub(crate) fn filters(chain: &[Filter]) -> Result<(), RejectReason> {
    for filter in chain {
        match *filter {
            Filter::Blur(deviation) => finite::numbers(&[deviation])?,
            Filter::DropShadow {
                offset,
                std_dev,
                color,
            } => {
                finite::numbers(&offset)?;
                finite::numbers(&[std_dev])?;
                finite::color(color)?;
            }
            Filter::ColorMatrix(matrix) => finite::numbers(&matrix)?,
            Filter::Opacity(amount) => finite::numbers(&[amount])?,
            Filter::Shader { reach, .. } => finite::numbers(&[reach])?,
        }
    }

    Ok(())
}

pub(crate) fn paint(paint: &Paint) -> Result<(), RejectReason> {
    match paint {
        Paint::Solid(color) => finite::color(*color),
        Paint::Sides(colors) => colors.iter().try_for_each(|color| finite::color(*color)),
        Paint::Gradient(gradient) => {
            match gradient.shape {
                GradientShape::Linear { start, end } => {
                    finite::numbers(&start)?;
                    finite::numbers(&end)?;
                }
                GradientShape::Radial { center, radius } => {
                    finite::numbers(&center)?;
                    finite::numbers(&radius)?;
                }
                GradientShape::Conic { center, angle } => {
                    finite::numbers(&center)?;
                    finite::numbers(&[angle])?;
                }
            }

            gradient.stops.iter().try_for_each(|stop| {
                finite::numbers(&[stop.offset, stop.color.alpha()])?;
                finite::numbers(&stop.color.components())
            })
        }
    }
}

/// Checks the pointer fields of a parameter block, the only numbers the compositor reads.
pub(crate) fn params(block: &ParamsBlock) -> Result<(), RejectReason> {
    finite::numbers(&block.pointer)?;
    finite::numbers(&[block.hovered])
}

/// Fails with `StaleId` when the props refer to a space or clip that is not live.
pub(crate) fn props_ids(client: &Client, props: &ChunkProps) -> Result<(), RejectReason> {
    live(&client.transforms, props.space)?;
    if let Some(clip) = props.clip {
        live(&client.clips, clip)?;
    }

    Ok(())
}

/// Fails with `InvalidItem` when the props hold an unlisted flag.
pub(crate) fn props_values(props: &ChunkProps) -> Result<(), RejectReason> {
    valid(props.flags.is_valid())
}

/// Fails with `StaleId` when a chunk-local clip refers to a parent node or mask image that is not
/// live.
pub(crate) fn local_clip_ids(client: &Client, clip: &LocalClip) -> Result<(), RejectReason> {
    if let LocalParent::Node(parent) = clip.parent {
        live(&client.clips, parent)?;
    }

    shape_ids(client, &clip.shape)
}

/// Fails with `InvalidItem` unless `ok`.
pub(crate) fn valid(ok: bool) -> Result<(), RejectReason> {
    if ok {
        Ok(())
    } else {
        Err(RejectReason::InvalidItem)
    }
}
