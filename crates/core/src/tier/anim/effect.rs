//! The value of an effect node with its animated properties applied.

use std::sync::Arc;

use iceblink::{AnimationTarget, EffectNode, Filter, FilterParam};

use crate::motion::value::Value;
use crate::tier::anim::Running;

/// Returns `base` with the opacity and filter parameters that `running` animate on effect
/// `slot` replaced by their sampled values.
pub(crate) fn animated_effect(base: &EffectNode, slot: u32, running: &[Running]) -> EffectNode {
    let mut node = base.clone();
    let mut filters: Option<Vec<Filter>> = None;
    for animation in running {
        let Some(value) = animation.value else {
            continue;
        };
        match (animation.animation.target, value) {
            (AnimationTarget::Opacity(id), Value::Scalar(opacity)) if id.slot() == slot => {
                node.opacity = opacity;
            }
            (
                AnimationTarget::Filter {
                    effect,
                    index,
                    param,
                },
                value,
            ) if effect.slot() == slot => {
                let filters = filters.get_or_insert_with(|| base.filters.to_vec());
                if let Some(filter) = filters.get_mut(usize::from(index)) {
                    set_param(filter, param, value);
                }
            }
            _ => {}
        }
    }

    if let Some(filters) = filters {
        node.filters = Arc::from(filters);
    }
    node
}

/// Reports whether the filter `filter` has the parameter `param`.
pub(crate) fn has_param(filter: &Filter, param: FilterParam) -> bool {
    matches!(
        (filter, param),
        (
            Filter::Blur(_) | Filter::DropShadow { .. } | Filter::Opacity(_),
            FilterParam::Amount
        ) | (Filter::DropShadow { .. }, FilterParam::Offset)
    )
}

/// Writes a sampled value into a filter's parameter.
fn set_param(filter: &mut Filter, param: FilterParam, value: Value) {
    match (filter, param, value) {
        (Filter::Blur(deviation), FilterParam::Amount, Value::Scalar(amount)) => {
            *deviation = amount
        }
        (Filter::DropShadow { std_dev, .. }, FilterParam::Amount, Value::Scalar(amount)) => {
            *std_dev = amount;
        }
        (Filter::Opacity(alpha), FilterParam::Amount, Value::Scalar(amount)) => *alpha = amount,
        (Filter::DropShadow { offset, .. }, FilterParam::Offset, Value::Vector(moved)) => {
            *offset = moved;
        }
        _ => {}
    }
}
