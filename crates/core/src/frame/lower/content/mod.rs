//! Lowering glyph runs, images, marks and paths into sprites, marks and paths over atlas tiles.

mod ink;

use iceblink::{
    ClipRef, GlyphRun, Image, Marks, Matrix4, PathFill, PathItem, PathStroke, SpaceRef,
};

use crate::content::{GlyphRunInput, Lowered, PathOwner, PathRoute, Target, keeps_pixels};
use crate::frame::lower::build::Build;
use crate::frame::lower::{Lowering, Slots};
use crate::scene::chunk::{Lane, LoweredItem, LoweringKey, Part, Placement, RunLowering};
use crate::shared_content::image_key;

pub(crate) use self::ink::glyph_run_ink;

/// Returns where an item drawn at `world` through its clip and space lands.
fn target(
    slots: &Slots<'_>,
    parts: &mut Lowering<'_>,
    world: Matrix4,
    base: [f32; 2],
    clip: ClipRef,
    space: SpaceRef,
) -> Target {
    Target {
        world,
        offset: base,
        clip: slots.clip(parts.tables, clip).0,
        space: slots.space(space).0,
        moving: slots.moving(slots.space_slot(space)),
    }
}

/// Returns the placement of content lowered under `world`, the world matrix of client space
/// `slot`, with sprites among it when `phased` is set.
fn placement(slots: &Slots<'_>, slot: u32, world: Matrix4, phased: bool) -> Placement {
    Placement {
        slot,
        world,
        moving: slots.moving(slot),
        phased,
    }
}

/// Lowers a glyph run into sprites over glyph tiles, and paths on the path route.
///
/// The run takes the lowering `earlier` instead when it was made with the same inputs and its
/// tiles are still there.
pub(super) fn glyph_run(
    slots: &Slots<'_>,
    parts: &mut Lowering<'_>,
    key: LoweringKey,
    build: &mut Build,
    (run, earlier): (&GlyphRun, Option<RunLowering>),
) -> LoweredItem {
    let chunk = slots.chunk;
    let (Some(glyphs), Some(coords)) = (
        chunk.glyphs.get(run.glyphs.indices()),
        chunk.coords.get(run.coords.indices()),
    ) else {
        return LoweredItem::Nothing;
    };

    let (slot, world) = slots.world(run.space);
    build.atlas = true;
    build.placed_under(placement(slots, slot, world, true));

    let mut item = *run;
    item.paint = slots.paint(parts.tables, run.paint);
    let subpixel = key.subpixel.is_some() && !key.isolated;
    let input = GlyphRunInput {
        run: &item,
        glyphs,
        coords,
        subpixel,
        output_scale: f32::from_bits(key.scale_bits),
    };

    let target = target(slots, parts, world, build.base, run.clip, run.space);
    let content = &mut *parts.content;
    let same = |earlier: &RunLowering| {
        moves_whole(earlier, &target)
            && earlier.subpixel == input.subpixel
            && earlier.scale_bits == key.scale_bits
            && earlier.paint == item.paint
            && earlier.epoch == content.epoch
            && earlier.evictions == content.cache.evictions()
    };
    let mut lowering = match earlier.filter(same) {
        Some(earlier) => earlier.lowering,
        None => {
            let mut lowering = crate::content::Lowering::default();
            content
                .cache
                .lower_glyph_run(&input, &target, &mut lowering);
            lowering
        }
    };

    build.tiles.append(&mut lowering.tiles);
    build.content(&mut lowering.items, target.clip, slot)
}

/// Reports whether the glyph run lowering `earlier` serves `target`: the targets are the same, or
/// differ only by a world matrix that moves every glyph by whole device pixels while the lowering
/// holds no outlines.
fn moves_whole(earlier: &RunLowering, target: &Target) -> bool {
    let before = &earlier.target;
    let at =
        |world: &Matrix4| Matrix4::translation(target.offset[0], target.offset[1], 0.0).then(world);
    // An outline is a frame path in device space, which the frame moves only from the world the
    // lowering is recorded under.
    let outlines = || {
        earlier
            .lowering
            .items
            .iter()
            .any(|item| matches!(item, Lowered::Path(_)))
    };
    before.offset == target.offset
        && before.clip == target.clip
        && before.space == target.space
        && before.moving == target.moving
        && (before.world == target.world
            || (!outlines() && keeps_pixels(&at(&before.world), &at(&target.world))))
}

/// Lowers an image into a colour sprite over the image's tile.
pub(super) fn image(
    slots: &Slots<'_>,
    parts: &mut Lowering<'_>,
    build: &mut Build,
    image: &Image,
) -> LoweredItem {
    let (slot, world) = slots.world(image.space);
    let mut item = *image;
    item.paint = slots.paint(parts.tables, image.paint);
    let target = target(slots, parts, world, build.base, image.clip, image.space);
    let key = image_key(parts.client, image.image);

    let mut lowering = crate::content::Lowering::default();
    let content = &mut *parts.content;
    if content
        .cache
        .lower_image(&content.images, key, &item, &target, &mut lowering)
        .is_err()
    {
        return LoweredItem::Nothing;
    }

    build.atlas = true;
    build.tiles.append(&mut lowering.tiles);
    build.content(&mut lowering.items, target.clip, slot)
}

/// Lowers a marks item into frame marks with its payload's shape cells.
pub(super) fn marks(
    slots: &Slots<'_>,
    parts: &mut Lowering<'_>,
    build: &mut Build,
    marks: &Marks,
) -> LoweredItem {
    let Some(payload) = slots.chunk.payloads.get(marks.payload as usize) else {
        return LoweredItem::Nothing;
    };

    let (slot, world) = slots.world(marks.space);
    let mut item = *marks;
    item.paint = slots.paint(parts.tables, marks.paint);
    let target = target(slots, parts, world, build.base, marks.clip, marks.space);
    let Ok(mut lowered) = parts.content.cache.lower_marks(&item, payload, &target) else {
        return LoweredItem::Nothing;
    };

    build.atlas = true;
    build.tiles.extend(lowered.tile);
    lowered.marks.marks.payload = build.chunk.mark_payloads.len() as u32;
    build.chunk.mark_payloads.push(lowered.payload);
    let start = build.chunk.marks.len() as u32;
    build.chunk.marks.push(lowered.marks);
    if lowered.paths.is_empty() {
        return LoweredItem::Instances {
            lane: Lane::Marks,
            start,
            len: 1,
        };
    }

    // Shapes too large for a cell are drawn as paths after the item's other prims.
    // Their anchors are quantised on the device, so they hold only on the same pixel phase.
    build.placed_under(placement(slots, slot, world, true));
    let first = build.parts.len() as u32;
    build.parts.push(Part::Instances {
        lane: Lane::Marks,
        start,
        len: 1,
    });
    for path in lowered.paths {
        build.path(path, target.clip, slot);
    }
    LoweredItem::Parts {
        start: first,
        len: build.parts.len() as u32 - first,
    }
}

/// Lowers a path item into mask sprites or a frame path, as the content cache routes it.
///
/// `owner` identifies the item across lowerings, for the history the mask cache keeps.
pub(super) fn path(
    slots: &Slots<'_>,
    parts: &mut Lowering<'_>,
    build: &mut Build,
    item: &PathItem,
    owner: PathOwner,
) -> LoweredItem {
    let (slot, world) = slots.world(item.space);
    let item = PathItem {
        fill: item.fill.map(|fill| PathFill {
            paint: slots.paint(parts.tables, fill.paint),
            rule: fill.rule,
        }),
        stroke: item.stroke.as_ref().map(|stroke| PathStroke {
            paint: slots.paint(parts.tables, stroke.paint),
            style: stroke.style.clone(),
        }),
        ..item.clone()
    };

    let target = target(slots, parts, world, build.base, item.clip, item.space);
    let mut lowering = crate::content::Lowering::default();
    let route = parts
        .content
        .cache
        .lower_path(&item, &target, owner, &mut lowering);
    if route == PathRoute::Nothing {
        return LoweredItem::Nothing;
    }

    build.atlas = true;
    build.placed_under(placement(slots, slot, world, route == PathRoute::Masks));
    build.tiles.append(&mut lowering.tiles);
    build.content(&mut lowering.items, target.clip, slot)
}
