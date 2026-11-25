//! Path items.

use iceblink::{PathItem, RejectReason};

use crate::naive::commit::check::finite;
use crate::naive::commit::check::items::scope::Scope;
use crate::naive::commit::check::nodes::valid;

pub(super) fn check(scope: &Scope<'_>, item: &PathItem) -> Result<(), RejectReason> {
    finite::path(&item.path)?;
    finite::affine(item.placement)?;
    finite::affine(item.brush)?;

    if let Some(fill) = &item.fill {
        valid(fill.rule.is_valid())?;
        scope.paint(fill.paint)?;
    }

    if let Some(stroke) = &item.stroke {
        finite::stroke(&stroke.style)?;
        scope.paint(stroke.paint)?;
    }

    for clip in item.clips.iter() {
        valid(clip.rule.is_valid())?;
        finite::path(&clip.path)?;
    }

    scope.clip(item.clip)?;
    scope.space(item.space)
}
