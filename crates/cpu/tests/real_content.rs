//! The CPU renderer against iceblink-wgpu on frames iceblink-core builds from real fonts and
//! from union marks.

mod common;

use std::sync::Arc;

use iceblink::kurbo::BezPath;
use iceblink::{
    ArenaRange, ClipRef, Color, FontData, FontId, Glyph, GlyphFlags, GlyphRun, MarkBox, MarkFlags,
    MarkPayload, MarkPlacement, Marks, Paint, PaintKind, PaintRef, SpaceRef, Transaction,
};
use iceblink::{Compositor as _, GlyphRaster as _};
use iceblink_testing::fonts::{COLOR, LATIN, read_font};
use iceblink_testing::golden::{RgbaImage, Tolerance, difference};
use iceblink_testing::gpu::device_lock;
use iceblink_text::SwashRaster;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

use crate::common::content::{Ids, SIDE, Stage, append, background, common, key};
use crate::common::targets::{on_cpu, on_gpu};

/// Glyph masks of every format, as the draw suite's sprite cases allow them to differ.
const GLYPHS: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 32,
};

/// Union and plain marks: two steps.
// Measured on NVIDIA and lavapipe: no pixel beyond 1 step.
const MARKS: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

#[test]
fn a_paragraph_of_mono_subpixel_and_colour_glyphs_draws_what_the_gpu_draws() {
    let _device = device_lock();
    let drawn = on_both(&|stage| {
        let latin = stage.compositor.font_registry().register(read_font(LATIN));
        let color = read_font(COLOR);
        let notes = colour_glyphs(&color);
        let color = stage.compositor.font_registry().register(color);
        stage.commit(|tx, ids| {
            background(tx, ids);
            let ink = tx.create_paint(Paint::Solid(Color([0.1, 0.1, 0.3, 1.0])));
            let mut chunk = tx.record(key(2), ids.props());
            let paint = chunk.paint_ref(PaintKind::SOLID, ink);
            let text = "The quick brown fox";
            let mono = line(LATIN, text, 13.0, [4.0, 20.0]);
            glyph_run(&mut chunk, latin, 13.0, &mono, paint, GlyphFlags::HINT);
            let striped = line(LATIN, text, 13.0, [4.0, 40.0]);
            glyph_run(
                &mut chunk,
                latin,
                13.0,
                &striped,
                paint,
                GlyphFlags::SUBPIXEL | GlyphFlags::HINT,
            );
            let placed: Vec<Glyph> = notes
                .iter()
                .enumerate()
                .map(|(index, &id)| Glyph {
                    id,
                    x: 8.0 + index as f32 * 28.0,
                    y: 90.0,
                })
                .collect();
            glyph_run(
                &mut chunk,
                color,
                24.0,
                &placed,
                paint,
                GlyphFlags::default(),
            );
            append(tx, ids, 1, &[key(2)]);
        });
    });
    assert_close("the paragraph", &drawn, GLYPHS);
}

#[test]
fn union_marks_draw_what_the_gpu_draws() {
    let _device = device_lock();
    let drawn = on_both(&|stage| {
        stage.commit(|tx, ids| {
            background(tx, ids);
            marks(tx, ids);
            append(tx, ids, 1, &[key(2)]);
        });
    });
    assert_close("the marks", &drawn, MARKS);
}

/// Draws the scene `write` commits on iceblink-wgpu and on the CPU renderer, with frames built
/// for what both draw, then moves its chunk, and returns both images of each frame.
fn on_both(write: &dyn Fn(&mut Stage)) -> Vec<(RgbaImage, RgbaImage)> {
    let gpu = on_gpu([SIDE, SIDE]).expect("iceblink-wgpu opens a device");
    let cpu = on_cpu([SIDE, SIDE]).expect("the CPU renderer opens");
    let mut targets = [gpu, cpu];
    let caps = common(
        targets[0].renderer().capabilities(),
        targets[1].renderer().capabilities(),
    );
    assert!(caps.subpixel_text, "both renderers draw subpixel text");
    let mut stages = targets.map(|target| Stage::new(target, caps));
    let mut frames = Vec::new();
    for stage in &mut stages {
        write(stage);
    }
    frames.push(stages.each_mut().map(Stage::draw));
    for stage in &mut stages {
        stage.commit(|tx, _| tx.set_offset(key(2), [3.5, 2.25]));
    }
    frames.push(stages.each_mut().map(Stage::draw));
    frames.into_iter().map(|[gpu, cpu]| (gpu, cpu)).collect()
}

#[track_caller]
fn assert_close(what: &str, drawn: &[(RgbaImage, RgbaImage)], tolerance: Tolerance) {
    for (frame, (gpu, cpu)) in drawn.iter().enumerate() {
        let found = difference(gpu, cpu, tolerance).expect("the images have one size");
        assert!(
            found.pixels <= tolerance.max_pixels,
            "{what}, frame {frame}: {} pixels differ by more than {}, the first at {:?}, by up \
             to {}",
            found.pixels,
            tolerance.max_channel,
            found.first,
            found.max_channel
        );
    }
}

/// Returns the glyphs of `text` set in the font `file` at `size`, from a pen at `origin` on the
/// baseline.
fn line(file: &str, text: &str, size: f32, origin: [f32; 2]) -> Vec<Glyph> {
    let font = read_font(file);
    let face = skrifa::FontRef::new(font.bytes()).expect("the font parses");
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(Size::new(size), LocationRef::default());
    let mut x = origin[0];
    text.chars()
        .map(|character| {
            let id = charmap.map(character).unwrap_or_default();
            let glyph = Glyph {
                id: id.to_u32(),
                x,
                y: origin[1],
            };
            x += metrics.advance_width(id).unwrap_or(size / 2.0);
            glyph
        })
        .collect()
}

/// Returns the first four glyphs of `font` that draw in colour.
fn colour_glyphs(font: &FontData) -> Vec<u32> {
    let raster = SwashRaster::new();
    let found: Vec<u32> = (1..256)
        .filter(|&id| raster.is_color(font, id))
        .take(4)
        .collect();
    assert_eq!(found.len(), 4, "the colour font has four colour glyphs");
    found
}

fn glyph_run(
    chunk: &mut iceblink::ChunkRecorder<'_>,
    font: FontId,
    size: f32,
    glyphs: &[Glyph],
    paint: PaintRef,
    flags: GlyphFlags,
) {
    let glyphs = chunk.glyphs(glyphs);
    chunk.glyph_run(GlyphRun {
        font,
        size,
        glyphs,
        coords: ArenaRange::default(),
        paint,
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
        _pad: [0; 1],
        palette: 0,
    });
}

/// Records chunk 2: discs, a box, a polyline and placed stars, drawn plain and as a union.
fn marks(tx: &mut Transaction, ids: Ids) {
    let purple = tx.create_paint(Paint::Solid(Color([0.4, 0.1, 0.5, 1.0])));
    let mut star = BezPath::new();
    star.move_to((0.0, -5.0));
    star.line_to((4.0, 4.0));
    star.line_to((-5.0, -2.0));
    star.line_to((5.0, -2.0));
    star.line_to((-4.0, 4.0));
    star.close_path();
    let payload = Arc::new(MarkPayload {
        discs: vec![[20.0, 20.0, 6.0, 0.0], [40.0, 20.0, 6.0, 3.0]],
        boxes: vec![MarkBox {
            rect: [60.0, 12.0, 80.0, 28.0],
            radii: [3.0; 8],
            shape: [2.0, 0.0, 0.0, 0.0],
        }],
        vertices: vec![
            [f32::NAN, 0.0],
            [10.0, 60.0],
            [40.0, 40.0],
            [70.0, 60.0],
            [f32::NAN, 0.0],
        ],
        shapes: vec![Arc::new(star)],
        placements: vec![
            MarkPlacement {
                x: 20.5,
                y: 90.25,
                shape: 0,
                _pad: 0,
            },
            MarkPlacement {
                x: 50.0,
                y: 90.0,
                shape: 0,
                _pad: 0,
            },
        ],
    });
    let mut chunk = tx.record(key(2), ids.props());
    let paint = chunk.paint_ref(PaintKind::SOLID, purple);
    let index = chunk.payload(payload);
    let marks = |flags: MarkFlags| Marks {
        bounds: [0.0, 0.0, 128.0, 128.0],
        paint,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 1.5,
        discs: 2,
        boxes: 1,
        vertices: 5,
        placements: 2,
        first: 0,
        payload: index,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
    };
    chunk.marks(marks(MarkFlags::default()));
    let mut united = marks(MarkFlags::UNION);
    united.origin = [0.0, 20.0];
    united.placements = 0;
    chunk.marks(united);
}
