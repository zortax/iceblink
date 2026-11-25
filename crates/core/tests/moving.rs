//! Glyph runs under a slight scale that writes keep changing.
//!
//! While the scale changes, the runs draw from tiles at their em size. Once it stops, they come to
//! rest on exactly rasterised tiles.

mod common;

use iceblink::damage::DamageSet;
use iceblink::geom::{Affine2, Device, Point, Rect, Size};
use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationStart, AnimationTarget, Built, ChunkKey, Direction, Easing, FillMode,
    Frame, FrameClock, FrameRequest, Keyframe, Keyframes, Matrix4, OrderEntry, PaintKind,
    RenderCapabilities, Timing, Transaction, TransformId, TransformNode,
};
use iceblink::{Compositor, Renderer};
use iceblink_testing::capture::CaptureRenderer;

use common::slice::Slice;
use common::story::World;
use common::text::run;
use common::{FULL, identity, props, stat};

type Captured = Slice<CaptureRenderer>;

const MS: u64 = 1_000_000;

/// The refresh interval of the frames drawn.
const FRAME: u64 = 16_666_667;

/// The chunk of the label.
const LABEL: ChunkKey = ChunkKey::new(1, 0);

/// How long a written node keeps moving after the frame before the write.
const HOLD: u64 = 100 * MS;

/// Capabilities that send every sprite in the frame's lanes.
const TRANSIENT: RenderCapabilities = RenderCapabilities {
    resident_chunks: false,
    ..FULL
};

/// Returns the local matrix of the menu at `scale`.
fn scaled(scale: f32) -> Matrix4 {
    Matrix4::scale(scale, scale, 1.0).then(&Matrix4::translation(10.3, 20.0, 0.0))
}

/// Returns a slice drawing a label under a new transform node with the local matrix `local`, past
/// its first frame at time zero.
fn menu(local: Matrix4) -> (Captured, TransformId) {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(TRANSIENT));
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let space = tx.create_transform(TransformNode {
        local,
        ..identity(Some(world.space))
    });
    world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    label(&mut tx, world, space);
    world.append(&mut tx, OrderEntry::Chunk(LABEL));
    slice.compositor.commit(tx).expect("the menu commits");
    assert!(draw_at(&mut slice, 0));
    (slice, space)
}

/// Records the label under `space`.
fn label(tx: &mut Transaction, world: &World, space: TransformId) {
    let mut chunk = tx.record(LABEL, props(space));
    let fill = chunk.paint_ref(PaintKind::SOLID, world.paints[0]);
    run(
        &mut chunk,
        world.font,
        "Export as CSV",
        14.0,
        [4.0, 30.0],
        fill,
    );
}

/// Writes the local matrix of `space` and records the label again, as a toolkit animating a
/// menu does.
fn write(slice: &mut Captured, space: TransformId, local: Matrix4) {
    let mut tx = slice.tx();
    tx.set_transform(space, local);
    label(&mut tx, &slice.world, space);
    slice.compositor.commit(tx).expect("the write commits");
}

/// Draws a frame presented at `present_ns`, and returns whether one was built.
fn draw_at(slice: &mut Captured, present_ns: u64) -> bool {
    slice.clock = clock(present_ns);
    slice.draw()
}

/// Builds a frame presented at `present_ns` that must be idle, and returns its wake time.
fn idle_at(slice: &mut Captured, present_ns: u64) -> Option<u64> {
    let caps = TRANSIENT;
    let request = FrameRequest {
        caps: &caps,
        age: slice.renderer.target_age(),
        clock: clock(present_ns),
        subpixel: None,
    };

    let mut frame = Frame::default();
    match slice
        .compositor
        .build_frame(slice.output, &request, &mut frame)
    {
        Built::Idle { wake_ns } => wake_ns,
        built => panic!(
            "expected an idle frame at {} ms, got {built:?}",
            present_ns / MS
        ),
    }
}

/// Returns a frame clock whose current and present times are both `present_ns`.
fn clock(present_ns: u64) -> FrameClock {
    FrameClock {
        now_ns: present_ns,
        present_ns,
        interval_ns: FRAME,
    }
}

/// Checks that the last frame damages every pixel its glyph sprites touch under `world`.
fn damaged(slice: &Captured, world: Matrix4) {
    within(&slice.frame.damage.draw, slice, world);
}

/// Checks that `damage` holds every pixel the glyph sprites of the last frame touch under
/// `world`.
fn within(damage: &DamageSet, slice: &Captured, world: Matrix4) {
    for sprite in &slice.frame.lanes.mono.items {
        let [x, y, width, height] = sprite.bounds;
        let [x0, y0, _] = world.transform_point(x, y, 0.0);
        let [x1, y1, _] = world.transform_point(x + width, y + height, 0.0);
        let pixels = Rect::new(
            Point::new(x0.floor() as i32, y0.floor() as i32),
            Size::new(
                (x1.ceil() - x0.floor()) as i32,
                (y1.ceil() - y0.floor()) as i32,
            ),
        );
        assert!(damage.contains(pixels), "{pixels:?} is damaged");
    }
}

/// Returns how many glyphs the compositor has rasterised.
fn rasterised(slice: &Captured) -> u64 {
    stat(&slice.compositor, "content.glyphs_rasterised")
}

/// Returns how many glyph sprites of the last frame are drawn from tiles at the em size, which
/// cover one local unit per texel, and how many from tiles rasterised for the device.
fn routes(slice: &Captured) -> (usize, usize) {
    let sprites = &slice.frame.lanes.mono.items;
    assert!(!sprites.is_empty(), "the frame draws glyph sprites");
    let resampled = sprites
        .iter()
        .filter(|sprite| {
            let texels = [sprite.tile.rect[2] as f32, sprite.tile.rect[3] as f32];
            (sprite.bounds[2] - texels[0]).abs() < 1.0e-3
                && (sprite.bounds[3] - texels[1]).abs() < 1.0e-3
        })
        .count();
    (resampled, sprites.len() - resampled)
}

#[test]
fn a_slight_scale_written_every_frame_draws_from_tiles_at_the_em_size() {
    let (mut slice, space) = menu(scaled(0.95));
    assert_eq!(routes(&slice).0, 0, "a scale at rest is rasterised exactly");

    write(&mut slice, space, scaled(0.955));
    assert!(draw_at(&mut slice, FRAME));
    let before = rasterised(&slice);
    for step in 2..8 {
        let local = scaled(0.95 + 0.005 * step as f32);
        write(&mut slice, space, local);
        assert!(draw_at(&mut slice, step * FRAME));
        assert_eq!(
            routes(&slice).1,
            0,
            "every glyph is resampled at step {step}"
        );
        damaged(&slice, local);
    }
    assert_eq!(
        rasterised(&slice),
        before,
        "no glyph is rasterised while the scale moves"
    );

    write(&mut slice, space, scaled(1.0));
    // The frame check holds the sprites under a translation to whole device pixels.
    assert!(draw_at(&mut slice, 8 * FRAME));
    assert_eq!(
        idle_at(&mut slice, 9 * FRAME),
        None,
        "nothing comes to rest"
    );
}

#[test]
fn a_slight_scale_comes_to_rest_damaged_and_measured_again() {
    let (mut slice, space) = menu(scaled(0.95));
    write(&mut slice, space, scaled(0.96));
    assert!(draw_at(&mut slice, FRAME));
    write(&mut slice, space, scaled(0.97));
    assert!(draw_at(&mut slice, 2 * FRAME));
    assert_eq!(routes(&slice).1, 0);

    let rest = FRAME + HOLD;
    assert_eq!(
        idle_at(&mut slice, 3 * FRAME),
        Some(rest),
        "the hold after the frame before the last write"
    );
    // The scene conformance case `glyphs_at_rest` checks what this frame draws.
    assert!(draw_at(&mut slice, rest), "coming to rest draws a frame");
    damaged(&slice, scaled(0.97));
    assert_eq!(idle_at(&mut slice, rest + FRAME), None, "nothing moves");

    let (mut resting, _) = menu(scaled(0.97));
    assert_eq!(
        hidden(&mut slice, rest + 2 * FRAME),
        hidden(&mut resting, FRAME),
        "the ink at rest is that of a scale never written"
    );
}

#[test]
fn a_frame_reports_the_damage_of_a_node_coming_to_rest_as_the_compositors_own() {
    let (mut slice, space) = menu(scaled(0.95));
    write(&mut slice, space, scaled(0.96));
    assert!(draw_at(&mut slice, FRAME));
    assert!(
        slice.frame.damage.driven.is_empty(),
        "a client's write is the client's damage"
    );

    assert!(draw_at(&mut slice, FRAME + HOLD));
    within(&slice.frame.damage.driven, &slice, scaled(0.96));

    write(&mut slice, space, scaled(0.96));
    assert!(draw_at(&mut slice, FRAME + HOLD + FRAME));
    assert!(slice.frame.damage.driven.is_empty());
}

/// Takes the menu out of the order tree, and returns the bounds of the damage that draws.
fn hidden(slice: &mut Captured, present_ns: u64) -> Option<Rect<i32, Device>> {
    let mut tx = slice.tx();
    tx.splice(slice.world.root, 0, 1, &[]);
    slice.compositor.commit(tx).expect("the splice commits");
    assert!(draw_at(slice, present_ns));
    slice.frame.damage.draw.bounds()
}

#[test]
fn a_large_scale_or_a_turn_written_every_frame_is_drawn_exactly() {
    let (mut slice, space) = menu(scaled(1.5));
    for step in 1..4 {
        write(&mut slice, space, scaled(1.5 + 0.05 * step as f32));
        assert!(slice.compositor.counters().commit.glyphs_rasterised > 0);
        assert!(draw_at(&mut slice, step * FRAME));
        assert_eq!(routes(&slice).0, 0, "every glyph is rasterised exactly");
    }
    assert_eq!(
        idle_at(&mut slice, 4 * FRAME),
        None,
        "nothing comes to rest"
    );

    let turned = |angle: f32| Affine2::rotation(angle).to_matrix4();
    let (mut slice, space) = menu(turned(0.1));
    for step in 1..4 {
        write(&mut slice, space, turned(0.1 + 0.05 * step as f32));
        assert!(draw_at(&mut slice, step * FRAME));
        assert!(
            slice.frame.lanes.mono.items.is_empty(),
            "a turned run is filled from outlines"
        );
    }
    assert_eq!(
        idle_at(&mut slice, 4 * FRAME),
        None,
        "nothing comes to rest"
    );
}

#[test]
fn translations_written_under_a_slight_scale_draw_exactly() {
    let (mut slice, space) = menu(scaled(0.95));
    for step in 1..5 {
        let moved = Matrix4::scale(0.95, 0.95, 1.0).then(&Matrix4::translation(
            10.3 + 0.3 * step as f32,
            20.0,
            0.0,
        ));
        write(&mut slice, space, moved);
        assert!(draw_at(&mut slice, step * FRAME));
        assert_eq!(routes(&slice).0, 0, "every glyph is rasterised exactly");
    }
    assert_eq!(idle_at(&mut slice, 5 * FRAME), None, "nothing moves");
}

#[test]
fn an_animated_slight_scale_draws_from_tiles_at_the_em_size() {
    let (mut slice, space) = menu(scaled(0.9));
    let mut tx = slice.tx();
    tx.animate(Animation {
        target: AnimationTarget::Transform(space),
        keyframes: Keyframes::Matrix(smallvec![
            Keyframe {
                offset: 0.0,
                value: scaled(0.9),
                easing: Easing::Linear,
            },
            Keyframe {
                offset: 1.0,
                value: scaled(1.0),
                easing: Easing::Linear,
            },
        ]),
        timing: Timing {
            duration_ns: 300 * MS,
            delay_ns: 0,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::Forwards,
            easing: Easing::Linear,
        },
        start: AnimationStart::At(0),
    });
    slice.compositor.commit(tx).expect("the animation commits");

    assert!(draw_at(&mut slice, FRAME));
    let before = rasterised(&slice);
    for step in 2..12 {
        assert!(draw_at(&mut slice, step * FRAME));
        assert_eq!(
            routes(&slice).1,
            0,
            "every glyph is resampled at step {step}"
        );
    }
    assert_eq!(rasterised(&slice), before);
}
