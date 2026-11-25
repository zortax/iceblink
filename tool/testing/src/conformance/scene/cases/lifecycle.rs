//! The frame lifecycle: frame ids, damage kept by outcomes that record nothing, recovery, idling,
//! the shader clock, and frames planned for renderers with fewer or more fast paths.

use std::sync::Arc;

use iceblink::kurbo::{BezPath, Circle, Rect as KurboRect, Shape};
use iceblink::{
    BufferAge, Built, ClipRef, FillRule, FrameClock, FrameOutcome, OrderEntry, PaintRef, PathFill,
    PathItem, RenderCapabilities, SkipReason, SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{device, key, pixels, record_quads};
use crate::conformance::scene::stage::{FULL, SIDE, Stage, covers, presented};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Frame ids of an output increase by one per built frame.
pub fn frame_ids(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut last = stage.frame.id.0;
    for step in 0..4 {
        stage.add_quads(key(step), &[([step as f32 * 10.0, 0.0, 8.0, 8.0], RED)]);
        assert_eq!(stage.draw(), Built::Frame);
        assert_eq!(stage.frame.id.0, last + 1, "frame ids increase by one");
        last = stage.frame.id.0;
    }

    assert!(matches!(stage.build(), Built::Idle { .. }));
    stage.add_quads(key(9), &[([90.0, 0.0, 8.0, 8.0], RED)]);
    assert_eq!(stage.draw(), Built::Frame);
    assert_eq!(
        stage.frame.id.0,
        last + 1,
        "an idle build takes no frame id"
    );
}

/// The damage of a frame that recorded nothing is drawn by the next frame.
pub fn kept_damage(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.add_quads(key(1), &[([10.0, 10.0, 20.0, 20.0], RED)]);
    let reasons = [
        SkipReason::Unconfigured,
        SkipReason::Unacquired,
        SkipReason::DeviceUnavailable,
    ];
    for (shade, reason) in reasons.into_iter().enumerate() {
        let color = [0, 60 * shade as u8, 255, 255];
        let mut tx = stage.tx();
        record_quads(
            &mut tx,
            key(1),
            stage.props(),
            &[([10.0, 10.0, 20.0, 20.0], color)],
        );
        stage.commit(tx);
        assert_eq!(stage.build(), Built::Frame);
        stage.done(FrameOutcome::Skipped(reason));

        assert_eq!(
            stage.build(),
            Built::Frame,
            "after {reason:?} the kept damage builds a frame"
        );
        assert!(
            covers(stage.damage(), pixels([10.0, 10.0, 20.0, 20.0])),
            "after {reason:?} the next frame redraws the kept damage: {:?}",
            stage.damage()
        );
        stage.done(presented());
    }
}

/// The first frame after `Recovered` redraws the whole output and replaces every table.
pub fn recovery(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.add_quads(key(1), &[([10.0, 10.0, 20.0, 20.0], RED)]);
    stage.draw();
    stage.add_quads(key(2), &[([40.0, 40.0, 20.0, 20.0], BLUE)]);
    assert_eq!(stage.build(), Built::Frame);
    stage.done(FrameOutcome::Recovered);

    assert_eq!(stage.build(), Built::Frame, "a recovery builds a frame");
    assert!(
        covers(stage.damage(), device(0, 0, SIDE as i32, SIDE as i32)),
        "the first frame after a recovery redraws the whole output: {:?}",
        stage.damage()
    );
    let tables = &stage.frame.tables;
    assert!(
        tables.spaces.replace
            && tables.clips.replace
            && tables.paints.replace
            && tables.params.replace,
        "the first frame after a recovery replaces every table"
    );
    assert_eq!(stage.picture.quads_of(RED).len(), 1);
    assert_eq!(stage.picture.quads_of(BLUE).len(), 1);
}

/// Frames planned for a renderer without resident chunks, shifts, kept rasters or subpixel text
/// use none of them.
pub fn minimal_renderers(make: MakeCompositor<'_>) {
    let caps = RenderCapabilities {
        max_texture_size: FULL.max_texture_size,
        ..RenderCapabilities::MINIMAL
    };
    // The frame check of every frame covers kept rasters and subpixel text.
    let mut stage = Stage::new(make, caps);
    stage.add_quads(key(1), &[([10.0, 10.0, 40.0, 40.0], RED)]);
    let mut tx = stage.tx();
    let mut chunk = tx.record(key(2), stage.props());
    let path: BezPath = KurboRect::new(60.0, 60.0, 90.0, 90.0)
        .path_elements(0.1)
        .collect();
    chunk.path(PathItem {
        path: Arc::new(path),
        placement: Default::default(),
        brush: Default::default(),
        fill: Some(PathFill {
            paint: PaintRef::color(BLUE),
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(2)));
    stage.commit(tx);
    stage.draw();
    assert!(
        stage.frame.drawn.is_empty() && stage.frame.chunk_notes.is_empty(),
        "a renderer without resident chunks gets no notes and draws nothing resident"
    );
    let mut tx = stage.tx();
    tx.set_offset(key(1), [3.0, 0.0]);
    stage.commit(tx);
    stage.draw();
    assert!(stage.frame.shifts.is_empty());
}

/// The shader clock counts seconds from the compositor's first build to each frame's present
/// time, and the time since the output's previous frame.
pub fn shader_clock(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let second = 1_000_000_000u64;
    let at = |present_ns: u64| FrameClock {
        now_ns: present_ns - 16_000_000,
        present_ns,
        interval_ns: 16_666_667,
    };
    stage.add_quads(key(1), &[([0.0, 0.0, 8.0, 8.0], RED)]);
    assert_eq!(stage.build_at(at(5 * second)), Built::Frame);
    stage.done(presented());
    let first = stage.frame.shader_clock;
    assert!(
        (first.time - 5.0).abs() < 1.0e-3,
        "the clock counts from the first build at zero: {first:?}"
    );

    stage.add_quads(key(2), &[([10.0, 0.0, 8.0, 8.0], BLUE)]);
    assert_eq!(stage.build_at(at(5 * second + second / 2)), Built::Frame);
    let next = stage.frame.shader_clock;
    assert!(
        (next.time - 5.5).abs() < 1.0e-3 && (next.delta - 0.5).abs() < 1.0e-3,
        "half a second later: {next:?}"
    );
}

/// Frames for a renderer that keeps vector rasters carry only keys the frame check accepts, and
/// a precise compositor plans every pass uncut to the damage.
pub fn pass_keys(make: MakeCompositor<'_>) {
    let caps = RenderCapabilities {
        kept_rasters: true,
        ..FULL
    };
    let mut stage = Stage::new(make, caps);
    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    let path: BezPath = Circle::new((50.0, 50.0), 30.0).path_elements(0.1).collect();
    chunk.path(PathItem {
        path: Arc::new(path),
        placement: Default::default(),
        brush: Default::default(),
        fill: Some(PathFill {
            paint: PaintRef::color(BLUE),
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.draw();

    for step in 0..3 {
        let left = 30.0 + step as f32 * 7.0;
        let mut tx = stage.tx();
        record_quads(
            &mut tx,
            key(2),
            stage.props(),
            &[([left, 70.0, 12.0, 12.0], RED)],
        );
        if step == 0 {
            stage.append(&mut tx, OrderEntry::Chunk(key(2)));
        }
        stage.commit(tx);
        assert_eq!(stage.draw(), Built::Frame);
        if stage.precise("pass_keys") {
            assert!(
                stage
                    .frame
                    .vector_plan
                    .passes
                    .iter()
                    .all(|pass| !pass.clamped),
                "a precise compositor plans passes uncut for a renderer that keeps rasters"
            );
        }
    }
}

/// An output idles when its frame's own damage is empty, even while the image it would draw into
/// is behind older frames.
pub fn idle_by_own_damage(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.add_quads(key(1), &[([0.0, 0.0, 16.0, 16.0], [255, 0, 0, 255])]);
    assert_eq!(
        stage.build_aged(BufferAge::Known(2)),
        Built::Frame,
        "a change builds a frame"
    );
    stage.done(presented());
    assert_eq!(
        stage.build_aged(BufferAge::Known(2)),
        Built::Idle { wake_ns: None },
        "nothing changed since, so the output idles although an older image lacks the change"
    );
}
