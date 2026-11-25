//! The inks of glyph runs, measured on their rasterised glyphs.

use iceblink::geom::Edges;
use iceblink::{ChunkKey, DeviceRect, FRAME_NO_CLIP, LocalRect, Matrix4};

use crate::content::{GlyphRunInput, Lowered, Lowering, Target, stretches, tiles_on_grid};
use crate::derive::diff::same_run;
use crate::frame::lower::{Slots, View};
use crate::geometry::bounds::{DeviceBox, INK_LIMIT, axis_scale, map, of_pixels};
use crate::output::Outputs;
use crate::scene::Scene;
use crate::scene::chunk::{ChunkRecord, RunInk, RunLowering};
use crate::shared_content::Content;
use crate::shared_content::prefetch::RunOutput;
use crate::tables::FrameTables;

/// What measuring glyph runs reads and updates besides the chunk.
pub(crate) struct Measure<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) tables: &'a mut FrameTables,
    pub(crate) content: &'a mut Content,
    pub(crate) outputs: &'a Outputs,
    /// What the output drawing the client allows its glyph runs.
    pub(crate) text: RunOutput,
}

/// Measures the ink of every glyph run of chunk `key` whose glyphs would land on other pixels or
/// phases than where they were last measured, or whose space started or stopped moving under a
/// matrix that stretches glyph tiles while it moves.
///
/// A run is measured on the glyphs it lowers to under its world matrix at the chunk's offset,
/// as its space's motion routes them. Its device ink is the pixels those glyphs cover. Its local ink is moved back by the offset and
/// grown by one device pixel for the glyph phases other translations give. A run under a
/// projective matrix, or drawn without a rasteriser, keeps a bound computed from its glyph
/// origins.
///
/// A measured run whose ink meets an output that draws the client, grown by the output's size on
/// every side, keeps its lowering, with frame slots and no context clip, for the chunk's next
/// lowering.
pub(crate) fn refresh(derive: &mut Measure<'_>, key: ChunkKey) {
    let scene = &*derive.scene;
    let Some(record) = scene.chunks.get(&key) else {
        return;
    };
    let chunk = &record.recorded;
    if chunk.glyph_runs.is_empty() || !derive.content.rasterises() {
        return;
    }

    let slots = Slots {
        scene: View::of(scene),
        chunk,
        locals: &record.derived.local_frames,
        context: FRAME_NO_CLIP,
    };
    let mut runs = record.derived.runs.clone();
    runs.resize(chunk.glyph_runs.len(), None);
    let mut kept: Vec<Option<RunLowering>> = Vec::new();

    for (index, run) in chunk.glyph_runs.iter().enumerate() {
        let (slot, world) = slots.world(run.space);
        let Some(affine) = world.to_affine2() else {
            runs[index] = None;
            continue;
        };
        let moving = slots.moving(slot);
        let held = runs[index].as_ref();
        let unmoved = |held: &RunInk| {
            held.moved_by(&world, record.offset).is_some()
                && (held.moving == moving || !stretches(&world))
        };
        if held.is_some_and(unmoved) {
            continue;
        }

        let (Some(glyphs), Some(coords)) = (
            chunk.glyphs.get(run.glyphs.indices()),
            chunk.coords.get(run.coords.indices()),
        ) else {
            continue;
        };
        // The run is lowered as the commit's prefetch rasterised it, so every glyph is cached.
        let mut item = *run;
        item.paint = slots.paint(derive.tables, run.paint);
        let subpixel = derive.text.subpixel.is_some();
        let input = GlyphRunInput {
            run: &item,
            glyphs,
            coords,
            subpixel,
            output_scale: derive.content.scale,
        };
        let target = Target {
            world,
            offset: record.offset,
            clip: slots.clip(derive.tables, run.clip).0,
            space: slots.space(run.space).0,
            moving,
        };
        let mut lowering = Lowering::default();
        derive
            .content
            .cache
            .lower_glyph_run(&input, &target, &mut lowering);

        let local = local_ink(
            &lowering,
            &world,
            record.offset,
            affine.a * affine.d - affine.b * affine.c,
        );
        let device = device_ink(&lowering, &world, moving);
        runs[index] = Some(RunInk {
            local,
            device,
            world,
            offset: record.offset,
            moving,
        });

        let drawn = local
            .is_some_and(|local| near_output(derive.outputs, scene, local, &world, record.offset));
        if drawn {
            kept.resize_with(chunk.glyph_runs.len(), || None);
            kept[index] = Some(RunLowering {
                target,
                subpixel,
                scale_bits: derive.content.scale.to_bits(),
                paint: item.paint,
                epoch: derive.content.epoch,
                evictions: derive.content.cache.evictions(),
                lowering,
            });
        }
    }

    if let Some(record) = derive.scene.chunks.get_mut(&key) {
        record.derived.runs = runs;
        record.derived.lowered_runs = kept;
    }
}

/// Reports whether a run with local ink `local` under `world` at `offset` meets an output that
/// draws the client, grown by the output's size on every side.
fn near_output(
    outputs: &Outputs,
    scene: &Scene,
    local: LocalRect,
    world: &Matrix4,
    offset: [f32; 2],
) -> bool {
    let mut rect = local;
    rect[0] += offset[0];
    rect[1] += offset[1];
    map(world, rect).pixels(INK_LIMIT).is_some_and(|ink| {
        outputs.values().any(|output| {
            output
                .root
                .is_some_and(|(client, _)| client == scene.client)
                && near(output.bounds()).intersects(ink)
        })
    })
}

/// Returns `bounds` grown by its own size on every side.
// A list records rows in the margin around its port, which scroll in a few frames later, and
// their lowering then serves the frame that draws them.
fn near(bounds: DeviceRect) -> DeviceRect {
    bounds.outset(Edges::axes(bounds.size.width, bounds.size.height))
}

/// Takes the measured ink of every glyph run of chunk `key` that is identical to the run at the
/// same index of `original`, the record the chunk replaced.
pub(crate) fn reuse(scene: &mut Scene, key: ChunkKey, original: &ChunkRecord) {
    let Some(record) = scene.chunks.get_mut(&key) else {
        return;
    };
    let chunk = &record.recorded;
    let runs: Vec<Option<RunInk>> = (0..chunk.glyph_runs.len())
        .map(|index| {
            let held = original.derived.runs.get(index).copied().flatten()?;
            same_run(&original.recorded, chunk, index).then_some(held)
        })
        .collect();
    if runs.iter().any(Option::is_some) {
        record.derived.runs = runs;
    }
}

/// Returns the device pixels a run's lowering under `world` covers, made while `moving`.
///
/// A sprite on whole device pixels covers exactly its bounds, a resampled sprite the pixels its
/// bounds touch, and a path its ink.
fn device_ink(lowering: &Lowering, world: &Matrix4, moving: bool) -> Option<DeviceRect> {
    let grid = tiles_on_grid(world, moving)
        .then(|| axis_scale(world))
        .flatten();
    let mut ink: Option<DeviceRect> = None;
    // A positive scale keeps the order of edges, so the sprites on the grid are mapped once, as
    // their union.
    let mut on_grid: Option<DeviceBox> = None;
    for item in &lowering.items {
        let bounds = match item {
            Lowered::Mono(sprite) | Lowered::Subpixel(sprite) => sprite.bounds,
            Lowered::Color(sprite) => sprite.bounds,
            Lowered::Path(path) => {
                ink = Some(ink.map_or(path.ink, |held| held.union(path.ink)));
                continue;
            }
        };
        if grid.is_some() {
            let local = DeviceBox::of(bounds);
            on_grid = Some(on_grid.map_or(local, |held| held.union(local)));
        } else if let Some(pixels) = map(world, bounds).pixels(INK_LIMIT) {
            ink = Some(ink.map_or(pixels, |held| held.union(pixels)));
        }
    }

    if let (Some([sx, sy, tx, ty]), Some(local)) = (grid, on_grid) {
        // The bounds were mapped back from whole device pixels, and mapping them forwards misses
        // those pixels by rounding only.
        let device = DeviceBox {
            x0: (sx * local.x0 + tx).round(),
            y0: (sy * local.y0 + ty).round(),
            x1: (sx * local.x1 + tx).round(),
            y1: (sy * local.y1 + ty).round(),
        };
        if let Some(pixels) = device.pixels(INK_LIMIT) {
            ink = Some(ink.map_or(pixels, |held| held.union(pixels)));
        }
    }
    ink
}

/// Returns the local ink of a run's lowering under `world` at `offset`, grown by one device pixel
/// for the glyph phases other offsets give.
///
/// `determinant` is the determinant of the linear part of `world`.
fn local_ink(
    lowering: &Lowering,
    world: &Matrix4,
    offset: [f32; 2],
    determinant: f32,
) -> Option<LocalRect> {
    // Only outlined glyphs are measured on the device, so most runs never need the inverse.
    let mut inverse: Option<Option<Matrix4>> = None;
    let mut ink: Option<DeviceBox> = None;
    for item in &lowering.items {
        let local = match item {
            Lowered::Mono(sprite) | Lowered::Subpixel(sprite) => DeviceBox::of(sprite.bounds),
            Lowered::Color(sprite) => DeviceBox::of(sprite.bounds),
            Lowered::Path(path) => match inverse.get_or_insert_with(|| world.invert()) {
                Some(inverse) => map(inverse, of_pixels(path.ink).rect()),
                None => continue,
            },
        };
        ink = Some(ink.map_or(local, |held| held.union(local)));
    }

    let scale = determinant.abs().sqrt().max(f32::EPSILON);
    let [x, y] = offset;
    let grow = 1.0 / scale;
    ink.map(|ink| ink.outset([grow + x, grow + y, grow - x, grow - y]).rect())
}
