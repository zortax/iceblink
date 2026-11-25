//! A scene with every item kind, node kind and clip kind.

mod items;
mod nodes;
mod resources;
mod shaders;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::{
    ArenaRange, ChunkKey, ChunkProps, ClipRef, FontId, Glyph, GlyphFlags, GlyphRun, OrderEntry,
    PaintRef, ParamsBlock, RenderCapabilities, SpaceRef,
};
use iceblink_testing::fonts::{COLOR, LATIN, read_font};
use iceblink_text::SwashRaster;

use crate::common::naive::{Harness, quad};

use self::items::Fonts;

/// Capabilities with every optional feature a renderer can report.
pub(crate) const FULL: RenderCapabilities = RenderCapabilities {
    max_texture_size: 8192,
    subpixel_text: true,
    custom_shaders: true,
    vector_compute: true,
    can_shift: true,
    resident_chunks: true,
    blend_modes: u32::MAX >> 15,
    kept_rasters: true,
};

/// The parameter block the shaders of the scene read.
pub(crate) fn params() -> ParamsBlock {
    ParamsBlock {
        pointer: [1.0, 2.0],
        hovered: 1.0,
        _reserved: 0.0,
        user: [3; 64],
    }
}

/// Builds the scene into `harness` as three chunks appended to the root.
///
/// Chunk 0 is a white background. Chunk 1 records every item kind under a scrolled, sticky and
/// fixed node, with node, local and mask clips, a group with filters, and a backdrop. Chunk 2
/// holds one subpixel glyph run.
pub(crate) fn build(harness: &mut Harness) {
    harness
        .compositor
        .use_glyph_raster(Arc::new(SwashRaster::new()));
    harness.add_quads(
        ChunkKey::new(0, 0),
        &[quad([0.0, 0.0, 96.0, 64.0], [255, 255, 255, 255])],
    );
    let registry = harness.compositor.font_registry();
    let fonts = Fonts {
        latin: registry.register(read_font(LATIN)),
        color: registry.register(read_font(COLOR)),
    };

    let mut transaction = harness.transaction();
    let nodes = nodes::create(&mut transaction, harness.space);
    let resources = resources::create(&mut transaction, harness.space, nodes.turned);

    let key = ChunkKey::new(1, 0);
    let props = ChunkProps {
        space: nodes.scrolled,
        clip: None,
        flags: Default::default(),
    };
    let mut recorder = transaction.record(key, props);
    items::record(&mut recorder, &nodes, &resources, fonts);

    transaction.splice(resources.clipped, 0, 0, &[OrderEntry::Chunk(key)]);
    transaction.splice(
        resources.group,
        0,
        0,
        &[OrderEntry::Node(resources.clipped)],
    );
    harness.append(&mut transaction, OrderEntry::Node(resources.group));
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    let props = harness.props();
    let mut recorder = transaction.record(ChunkKey::new(2, 0), props);
    let range = recorder.glyphs(&glyphs());
    recorder.glyph_run(glyph_run(
        fonts.latin,
        range,
        PaintRef::color([0, 0, 0, 255]),
        GlyphFlags::SUBPIXEL,
        SpaceRef::INHERIT,
    ));
    harness.append(&mut transaction, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    harness.commit(transaction);
}

/// Moves chunk 1 of the scene by a fractional offset.
pub(crate) fn nudge(harness: &mut Harness) {
    let mut transaction = harness.transaction();
    transaction.set_offset(ChunkKey::new(1, 0), [1.25, -3.5]);
    harness.commit(transaction);
}

/// Six glyphs on one baseline, at fractional positions.
fn glyphs() -> Vec<Glyph> {
    (0..6)
        .map(|index| Glyph {
            id: 30 + index,
            x: 3.6 + 8.0 * index as f32,
            y: 50.0,
        })
        .collect()
}

/// A run of `glyphs` at 12 pixels, emboldened and skewed.
fn glyph_run(
    font: FontId,
    glyphs: ArenaRange,
    paint: PaintRef,
    flags: GlyphFlags,
    space: SpaceRef,
) -> GlyphRun {
    GlyphRun {
        font,
        size: 12.0,
        glyphs,
        coords: ArenaRange::default(),
        paint,
        embolden: 0.5,
        skew: 8.0,
        clip: ClipRef::INHERIT,
        space,
        flags,
        _pad: [0; 1],
        palette: 0,
    }
}
