//! Damage of the items that refer to a written slot or resource.

use iceblink::{
    ChunkKey, DeviceRect, Filter, ItemKind, ItemOp, PaintKind, PaintRef, RecordedChunk,
};
use smallvec::SmallVec;

use crate::commit::journal::{Journal, Op};
use crate::derive::Derive;
use crate::derive::order::exact_ink;
use crate::derive::route::{Routes, damage};
use crate::scene::kind::{Kind, ObjectRef};

/// Damages the items that refer to a paint or parameter slot the commit wrote.
pub(super) fn slots(derive: &mut Derive<'_>, journal: &Journal, routes: &Routes) {
    let mut written: SmallVec<[ObjectRef; 8]> = SmallVec::new();
    for change in &journal.paints {
        if matches!(change.op, Op::Wrote(_)) {
            written.push(ObjectRef::new(Kind::Paint, change.slot));
        }
    }
    for change in &journal.params {
        if matches!(change.op, Op::Wrote(_)) {
            written.push(ObjectRef::new(Kind::Params, change.slot));
        }
    }
    written.sort_unstable();
    written.dedup();

    for &object in &written {
        damage_users(derive, object, routes);
        if object.kind == Kind::Params {
            damage_filter_users(derive, object.slot, routes);
        }
    }
}

/// Damages the written region of every group whose filters read the parameter slot `params`.
pub(super) fn damage_filter_users(derive: &mut Derive<'_>, params: u32, routes: &Routes) {
    let mut nodes: SmallVec<[u32; 4]> = SmallVec::new();
    for &effect in &derive.scene.shaded_effects {
        let Some(record) = derive.scene.effects.at(effect) else {
            continue;
        };
        let reads = record.node.filters.iter().any(
            |filter| matches!(filter, Filter::Shader { params: named, .. } if named.slot() == params),
        );
        if reads {
            nodes.extend(record.orders.iter().copied());
        }
    }

    for node in nodes {
        let ink = exact_ink(derive.scene, node);
        let route = routes.of_group(derive.scene, node);
        damage(derive.outputs, route, ink);
    }
}

/// Damages every item that refers to `object`, in every chunk that refers to it.
pub(crate) fn damage_users(derive: &mut Derive<'_>, object: ObjectRef, routes: &Routes) {
    let Some(links) = derive.scene.links(object) else {
        return;
    };
    let users: SmallVec<[ChunkKey; 8]> = links.users.iter().copied().collect();
    for key in users {
        let Some(record) = derive.scene.chunks.get(&key) else {
            continue;
        };
        let route = routes.of_chunk(derive.scene, key);
        let inks = derive.scene.device_inks(&record.derived.inks);
        let mut rects: SmallVec<[DeviceRect; 8]> = SmallVec::new();
        if object.kind == Kind::Clip {
            // A mask clip changes every item drawn through it.
            rects.extend(inks.union());
        }
        for (position, op) in record.recorded.ops.iter().enumerate() {
            if names(&record.recorded, *op, object) {
                rects.extend(inks.item(position));
            }
        }
        for rect in rects {
            damage(derive.outputs, route, Some(rect));
        }
    }
}

/// Reports whether the item `op` refers to `object` through one of its slots.
pub(super) fn names(chunk: &RecordedChunk, op: ItemOp, object: ObjectRef) -> bool {
    let index = op.index as usize;
    let paint = |paint: PaintRef| {
        object.kind == Kind::Paint
            && !matches!(paint.kind, PaintKind::NONE | PaintKind::INLINE)
            && paint.index == object.slot
    };
    match op.kind {
        ItemKind::QUAD => chunk
            .quads
            .get(index)
            .is_some_and(|i| paint(i.fill) || paint(i.stroke)),
        ItemKind::SHADER_QUAD => chunk.shader_quads.get(index).is_some_and(|i| {
            paint(i.fill)
                || paint(i.stroke)
                || (object.kind == Kind::Params && i.params == object.slot)
                || (object.kind == Kind::Shader && i.shader == object.slot)
        }),
        ItemKind::GLYPH_RUN => chunk.glyph_runs.get(index).is_some_and(|i| paint(i.paint)),
        ItemKind::IMAGE => chunk.images.get(index).is_some_and(|i| {
            paint(i.paint) || (object.kind == Kind::Image && i.image == object.slot)
        }),
        ItemKind::MARKS => chunk.marks.get(index).is_some_and(|i| paint(i.paint)),
        ItemKind::PATH => chunk.paths.get(index).is_some_and(|i| {
            i.fill.as_ref().is_some_and(|fill| paint(fill.paint))
                || i.stroke.as_ref().is_some_and(|stroke| paint(stroke.paint))
        }),
        ItemKind::EXTERNAL => chunk
            .externals
            .get(index)
            .is_some_and(|i| object.kind == Kind::Texture && i.texture == object.slot),
        ItemKind::BACKDROP => chunk.backdrops.get(index).is_some_and(|i| {
            chunk.filters.get(i.filters as usize).is_some_and(|chain| {
                chain.iter().any(|filter| match filter {
                    Filter::Shader { shader, params, .. } => {
                        (object.kind == Kind::Shader && shader.slot() == object.slot)
                            || (object.kind == Kind::Params && params.slot() == object.slot)
                    }
                    _ => false,
                })
            })
        }),
        _ => false,
    }
}
