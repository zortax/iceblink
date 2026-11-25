//! Rasterising the glyphs of newly recorded chunks at commit.

use iceblink::{ChunkKey, DeviceRect, GlyphRun, Matrix4, SpaceRef, SubpixelOrder};

use crate::content::{GlyphRunInput, RunJob, Target};
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::shared_content::Content;

pub(crate) use crate::shared_content::opaque::opaque_ref;

/// What an output allows the glyph runs it draws.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RunOutput {
    /// The stripe order subpixel sprites use, when the output may draw them.
    pub(crate) subpixel: Option<SubpixelOrder>,
    /// Device pixels per logical pixel.
    pub(crate) scale: f32,
}

/// Rasterises every glyph the glyph runs of `keys` draw that the cache does not hold yet, in
/// parallel when there are enough of them.
pub(crate) fn prefetch(content: &mut Content, scene: &Scene, keys: &[ChunkKey], output: RunOutput) {
    let mut jobs = Vec::new();
    for key in keys {
        let Some(record) = scene.chunks.get(key) else {
            continue;
        };
        jobs.extend(
            record
                .recorded
                .glyph_runs
                .iter()
                .filter_map(|run| run_job(scene, record, run, output)),
        );
    }

    if !jobs.is_empty() {
        content.cache.prefetch_glyph_runs(&jobs);
    }
}

/// Returns the job of `run` in the chunk of `record`, or `None` when its glyphs or coordinates lie
/// outside the chunk's arenas.
fn run_job<'a>(
    scene: &Scene,
    record: &'a ChunkRecord,
    run: &'a GlyphRun,
    output: RunOutput,
) -> Option<RunJob<'a>> {
    let chunk = &record.recorded;
    let glyphs = chunk.glyphs.get(run.glyphs.indices())?;
    let coords = chunk.coords.get(run.coords.indices())?;

    let space = if run.space == SpaceRef::INHERIT {
        chunk.props.space.slot()
    } else {
        run.space.0
    };
    let (world, moving) = scene
        .transforms
        .at(space)
        .map_or((Matrix4::IDENTITY, false), |record| {
            (record.world, record.moving)
        });
    let mut target = Target::new(world);
    target.offset = record.offset;
    target.moving = moving;

    let subpixel = output.subpixel.is_some();
    Some(RunJob {
        input: GlyphRunInput {
            run,
            glyphs,
            coords,
            subpixel,
            output_scale: output.scale,
        },
        target,
    })
}

/// Rasterises the glyphs of every chunk with glyph runs whose ink meets `bounds`, in parallel
/// when there are enough of them.
pub(crate) fn prefetch_drawn(
    content: &mut Content,
    scene: &Scene,
    bounds: DeviceRect,
    output: RunOutput,
) {
    let keys: Vec<ChunkKey> = scene
        .chunks
        .iter()
        .filter(|(_, record)| {
            !record.recorded.glyph_runs.is_empty()
                && scene
                    .device_inks(&record.derived.inks)
                    .union()
                    .is_some_and(|ink| ink.intersects(bounds))
        })
        .map(|(key, _)| *key)
        .collect();
    prefetch(content, scene, &keys, output);
}
