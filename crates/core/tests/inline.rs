//! Inline colours: lowered into shared frame paint slots, and solid wherever a solid paint is.

mod common;

use iceblink::Compositor;
use iceblink::{
    Built, ChunkKey, Color, FramePaint, GlyphFlags, OrderEntry, Paint, PaintKind, PaintRef,
    RenderCapabilities, SubpixelOrder,
};
use iceblink_testing::capture::CaptureRenderer;

use common::slice::Slice;
use common::text::run_with;
use common::{FULL, Scene, props, quad, rect, stat};

/// A renderer without resident chunks, so every drawn quad is in the frame's lanes.
const TRANSIENT: RenderCapabilities = RenderCapabilities {
    resident_chunks: false,
    ..FULL
};

/// A scene whose frames carry their quads, with the frame paint table the renderer holds.
struct Drawn {
    scene: Scene,
    paints: Vec<FramePaint>,
}

impl Drawn {
    fn new() -> Self {
        Self {
            scene: Scene::with_caps(TRANSIENT),
            paints: Vec::new(),
        }
    }

    /// Records chunk `key` with one quad per rectangle filled with `fill`, and appends it to the
    /// root when `place` is set.
    fn record(&mut self, key: ChunkKey, fill: PaintRef, rects: &[[f32; 4]], place: bool) {
        let scene = &mut self.scene;
        let mut tx = scene.tx();
        let mut chunk = tx.record(key, props(scene.space));
        for &bounds in rects {
            chunk.quad(quad(bounds, fill));
        }
        if place {
            tx.splice(scene.root, scene.entries, 0, &[OrderEntry::Chunk(key)]);
            scene.entries += 1;
        }
        scene.commit(tx);
    }

    /// Draws a frame, and returns the fill of each quad it draws with the paint in its slot.
    fn draw(&mut self) -> Vec<(PaintRef, FramePaint)> {
        assert_eq!(self.scene.draw(), Built::Frame);
        self.scene.frame.tables.paints.apply(&mut self.paints);
        self.scene
            .frame
            .lanes
            .quads
            .items
            .iter()
            .map(|quad| {
                let held = self.paints.get(quad.fill.index as usize).cloned();
                (quad.fill, held.unwrap_or_default())
            })
            .collect()
    }
}

#[test]
fn an_inline_colour_draws_as_a_solid_slot_holding_the_colour() {
    let mut drawn = Drawn::new();
    let fill = PaintRef::color([255, 0, 51, 255]);
    drawn.record(ChunkKey::new(1, 0), fill, &[[10.0, 10.0, 20.0, 20.0]], true);

    let fills = drawn.draw();
    assert_eq!(fills.len(), 1);
    let (lowered, paint) = &fills[0];
    assert_eq!(lowered.kind, PaintKind::SOLID);
    assert_eq!(*paint, FramePaint::Solid(Color([1.0, 0.0, 0.2, 1.0])));
}

#[test]
fn equal_inline_colours_share_one_slot_that_lasts_while_a_recording_uses_it() {
    let mut drawn = Drawn::new();
    let fill = PaintRef::color([0, 128, 0, 255]);
    let first = ChunkKey::new(1, 0);
    drawn.record(first, fill, &[[10.0, 10.0, 10.0, 10.0]], true);
    let slot = drawn.draw()[0].0;

    // The new recording takes the colour before the old one drops it.
    drawn.record(first, fill, &[[10.0, 30.0, 10.0, 10.0]], false);
    let again = drawn.draw();
    assert!(again.iter().all(|(lowered, _)| *lowered == slot));

    drawn.record(ChunkKey::new(2, 0), fill, &[[40.0, 10.0, 10.0, 10.0]], true);
    let both = drawn.draw();
    assert_eq!(both.len(), 1, "only the new chunk is damaged");
    assert_eq!(both[0].0, slot, "both chunks name one slot");
}

#[test]
fn the_slots_of_colours_no_recording_uses_are_reused() {
    let mut drawn = Drawn::new();
    let key = ChunkKey::new(1, 0);
    let mut highest = 0;
    for step in 0..64_u8 {
        let fill = PaintRef::color([step, 0, 0, 255]);
        drawn.record(key, fill, &[[10.0, 10.0, 10.0, 10.0]], step == 0);
        // An identical recording replaces the one it equals.
        drawn.record(key, fill, &[[10.0, 10.0, 10.0, 10.0]], false);
        let fills = drawn.draw();
        assert_eq!(
            fills[0].1,
            FramePaint::Solid(Color([f32::from(step) / 255.0, 0.0, 0.0, 1.0]))
        );
        highest = highest.max(fills[0].0.index);
    }
    assert!(highest < 4, "64 colours in turn used slots up to {highest}");
}

#[test]
fn removing_a_client_releases_the_slots_of_its_inline_colours() {
    let mut drawn = Drawn::new();
    drawn.record(
        ChunkKey::new(1, 0),
        PaintRef::color([9, 9, 9, 255]),
        &[[10.0, 10.0, 10.0, 10.0]],
        true,
    );
    let slot = drawn.draw()[0].0.index;
    let scene = &mut drawn.scene;
    scene.compositor.unregister_client(scene.client);
    assert_eq!(scene.build(), Built::Frame);
    scene.done(common::presented());

    // Another client draws while the released slot waits out the frame that last named it.
    let (client, _) = scene.compositor.register_client();
    scene.client = client;
    let mut tx = scene.tx();
    scene.space = tx.create_transform(common::identity(None));
    scene.root = tx.create_order(iceblink::OrderNode::default());
    tx.set_root(scene.output, Some(scene.root));
    scene.commit(tx);
    scene.entries = 0;
    let other = PaintRef::color([1, 2, 3, 255]);
    drawn.record(
        ChunkKey::new(1, 0),
        other,
        &[[10.0, 10.0, 10.0, 10.0]],
        true,
    );
    drawn.draw();

    let reused = PaintRef::color([4, 5, 6, 255]);
    drawn.record(
        ChunkKey::new(2, 0),
        reused,
        &[[40.0, 10.0, 10.0, 10.0]],
        true,
    );
    let fills = drawn.draw();
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].0.index, slot);
}

#[test]
fn a_paint_write_does_not_damage_an_inline_colour_that_packs_the_slot_number() {
    let mut scene = Scene::with_caps(TRANSIENT);
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 0.0, 0.0, 1.0])));
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([10.0, 10.0, 10.0, 10.0], fill));
    let packed = PaintRef::color(fill.index.to_le_bytes());
    chunk.quad(quad([50.0, 10.0, 10.0, 10.0], packed));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_paint(paint, Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(10, 10, 10, 10)]));
}

#[test]
fn an_opaque_inline_fill_is_an_opaque_cover() {
    let mut drawn = Drawn::new();
    let opaque = PaintRef::color([20, 40, 60, 255]);
    drawn.record(
        ChunkKey::new(1, 0),
        opaque,
        &[[0.0, 0.0, 200.0, 200.0]],
        true,
    );
    drawn.draw();
    assert_eq!(drawn.scene.frame.opaque_covers.len(), 1);

    let translucent = PaintRef::color([10, 20, 30, 128]);
    drawn.record(
        ChunkKey::new(1, 0),
        translucent,
        &[[0.0, 0.0, 200.0, 200.0]],
        false,
    );
    drawn.draw();
    assert!(drawn.scene.frame.opaque_covers.is_empty());
}

#[test]
fn recording_an_inline_colour_again_damages_only_where_it_changed() {
    let mut scene = Scene::with_caps(TRANSIENT);
    let key = ChunkKey::new(1, 0);
    let record = |scene: &mut Scene, second: [u8; 4], place: bool| {
        let mut tx = scene.tx();
        let mut chunk = tx.record(key, props(scene.space));
        chunk.quad(quad(
            [10.0, 10.0, 10.0, 10.0],
            PaintRef::color([255, 0, 0, 255]),
        ));
        chunk.quad(quad([50.0, 10.0, 10.0, 10.0], PaintRef::color(second)));
        if place {
            tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
        }
        scene.commit(tx);
    };
    record(&mut scene, [0, 0, 255, 255], true);
    scene.draw();

    record(&mut scene, [0, 0, 255, 255], false);
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });

    record(&mut scene, [0, 255, 0, 255], false);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(50, 10, 10, 10)]));
}

#[test]
fn a_run_with_an_inline_colour_is_prefetched_at_commit_into_subpixel_sprites() {
    let caps = RenderCapabilities {
        subpixel_text: true,
        ..FULL
    };
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(caps));
    slice.subpixel = Some(SubpixelOrder::RedToBlue);
    slice.draw();
    let text: String = (0x21_u8..0x7f).map(char::from).collect();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(world.space));
    let fill = PaintRef::color([0, 0, 0, 255]);
    let flags = GlyphFlags::SUBPIXEL;
    for size in [10.0, 11.0, 12.0] {
        let origin = [0.0, size * 4.0];
        run_with(&mut chunk, world.font, &text, size, origin, fill, flags);
    }
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the runs commit");

    assert!(
        stat(&slice.compositor, "content.prefetch_batches") > 0,
        "the commit rasterises the runs' atlas glyphs in parallel"
    );
    let rasterised = stat(&slice.compositor, "content.glyphs_rasterised");
    assert!(slice.draw());
    assert_eq!(
        stat(&slice.compositor, "content.glyphs_rasterised"),
        rasterised
    );
    assert!(!slice.frame.lanes.subpixel.sources.is_empty());
    assert!(slice.frame.lanes.mono.sources.is_empty());
}

#[test]
fn an_inline_colour_transcribes_as_the_solid_slot_it_stands_for() {
    let transcript = |inline: bool| {
        let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
        slice.draw();
        let mut tx = slice.tx();
        let world = &mut slice.world;
        let slot = (!inline).then(|| world.solid(&mut tx, [1.0, 0.0, 0.2, 1.0]));
        let key = ChunkKey::new(1, 0);
        let mut chunk = tx.record(key, props(world.space));
        let fill = match slot {
            Some(slot) => chunk.paint_ref(PaintKind::SOLID, slot),
            None => PaintRef::color([255, 0, 51, 255]),
        };
        chunk.quad(quad([10.0, 10.0, 20.0, 20.0], fill));
        world.append(&mut tx, OrderEntry::Chunk(key));
        slice.compositor.commit(tx).expect("the quad commits");
        assert!(slice.draw());
        let transcript = slice.renderer.transcript().expect("a frame was drawn");
        transcript.as_str().to_owned()
    };
    let inline = transcript(true);
    assert!(inline.contains("solid premul"), "{inline}");
    assert_eq!(inline, transcript(false));
}
