//! Presenting into textures the application owns and rotates between.
//!
//! Ordinary textures stand in for a display's scanout buffers. A frame lands on the texture the
//! application chose and on no other, and each texture receives its debt before a frame lands on
//! it.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::build::{FrameBuilder, area, filled, opaque, rect};
use common::device::{SIDE, device_lock};
use common::present::{Rig, target};
use iceblink::Renderer as _;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{Color, FrameOutcome, FramePaint, PaintRef, Shift, SkipReason};
use iceblink_wgpu::{Gpu, SharedGraphics, SrgbTier, Supplied, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// The usage of every supplied texture.
// `COPY_SRC` is for the readbacks only.
const USAGE: wgpu::TextureUsages =
    wgpu::TextureUsages::RENDER_ATTACHMENT.union(wgpu::TextureUsages::COPY_SRC);

/// Where the square of [`Square`] is drawn.
const SQUARE: [f32; 4] = [48.0, 48.0, 32.0, 32.0];

/// A named change to a supplied texture's descriptor.
type Alteration = (&'static str, fn(&mut wgpu::TextureDescriptor<'_>));

/// Returns a supplied texture of the output's side and [`FORMAT`], with its descriptor changed by
/// `alter`.
fn described(gpu: &Gpu, alter: impl FnOnce(&mut wgpu::TextureDescriptor<'_>)) -> wgpu::Texture {
    let mut descriptor = wgpu::TextureDescriptor {
        label: Some("test.scanout"),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: USAGE,
        view_formats: &[],
    };
    alter(&mut descriptor);
    gpu.device().create_texture(&descriptor)
}

/// Returns a supplied texture of `side` and `format`.
fn buffer(gpu: &Gpu, side: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    described(gpu, |descriptor| {
        descriptor.size.width = side;
        descriptor.size.height = side;
        descriptor.format = format;
    })
}

/// Opens the device of `graphics`.
fn open(graphics: &SharedGraphics) -> Arc<Gpu> {
    graphics
        .open_gpu()
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"))
}

/// Returns a rig over `count` supplied textures of `format`, and the textures.
fn supplied_in(count: usize, format: wgpu::TextureFormat) -> (Rig, Vec<wgpu::Texture>) {
    let guard = device_lock();
    let graphics = SharedGraphics::new();
    let gpu = open(&graphics);
    let textures: Vec<wgpu::Texture> = (0..count).map(|_| buffer(&gpu, SIDE, format)).collect();
    let renderer = Supplied::renderer(&graphics, textures.clone())
        .expect("a device is open and the textures agree about everything");
    (Rig::new(renderer, guard), textures)
}

/// Returns a rig over `count` supplied textures of [`FORMAT`], and the textures.
fn supplied(count: usize) -> (Rig, Vec<wgpu::Texture>) {
    supplied_in(count, FORMAT)
}

/// Returns the pixel at the centre of `texture`.
fn centre(rig: &Rig, texture: &wgpu::Texture) -> [u8; 4] {
    rig.read(texture).rgba(SIDE as i32 / 2, SIDE as i32 / 2)
}

/// Returns a first frame of one quad of `color` over the whole output.
fn filled_with(color: Color) -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(color);
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), paint));
    builder.finish()
}

/// The paints of a dark background and a square, which later frames recolour.
struct Square {
    back: PaintRef,
    front: PaintRef,
}

impl Square {
    /// Returns the first frame: the square of `color` on the background, drawn whole.
    fn first(color: Color) -> (Self, iceblink::Frame) {
        let mut builder = FrameBuilder::new();
        let back = builder.solid(opaque(20, 20, 20));
        let front = builder.solid(color);
        let square = Self { back, front };
        square.draw(&mut builder, SQUARE);
        (square, builder.finish())
    }

    /// Returns a later frame that recolours the square to `color`, moved to `at`, with `damage`.
    fn next(&self, color: Color, at: [f32; 4], damage: DamageSet) -> iceblink::Frame {
        let mut builder = FrameBuilder::next().damage(damage);
        builder.set_paint(self.front.index, FramePaint::Solid(color));
        self.draw(&mut builder, at);
        builder.finish()
    }

    /// Adds the background and the square at `at`.
    fn draw(&self, builder: &mut FrameBuilder, at: [f32; 4]) {
        builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), self.back));
        builder.quad(filled(at, self.front));
    }
}

/// Draws `first` into the first texture, and the same picture whole into each other one.
fn whole_into_each(rig: &mut Rig, count: usize, square: &Square, first: iceblink::Frame) {
    rig.select(0);
    assert!(rig.draw(&first).stats().is_some());
    for slot in 1..count as u32 {
        rig.select(slot);
        let frame = square.next(opaque(0, 0, 0), SQUARE, DamageSet::full());
        assert!(rig.draw(&frame).stats().is_some());
    }
}

/// Returns the damage of [`SQUARE`].
fn square_damage() -> DamageSet {
    let [x, y, width, height] = SQUARE;
    area(x as i32, y as i32, width as i32, height as i32)
}

#[test]
fn a_frame_lands_in_the_slot_the_application_chose_and_in_no_other() {
    let (mut rig, textures) = supplied(2);
    rig.select(0);
    rig.present(&filled_with(opaque(255, 0, 0)));
    rig.select(1);
    rig.present(&filled_with(opaque(0, 0, 255)));

    assert_eq!(centre(&rig, &textures[0]), [255, 0, 0, 255]);
    assert_eq!(centre(&rig, &textures[1]), [0, 0, 255, 255]);

    rig.select(0);
    rig.present(&filled_with(opaque(0, 255, 0)));

    assert_eq!(centre(&rig, &textures[0]), [0, 255, 0, 255]);
    assert_eq!(
        centre(&rig, &textures[1]),
        [0, 0, 255, 255],
        "returning to the first texture disturbed the second"
    );
}

#[test]
fn a_slot_outside_the_set_is_refused_and_the_frame_goes_where_it_was_going() {
    let (mut rig, textures) = supplied(2);
    rig.select(0);
    rig.present(&filled_with(opaque(255, 0, 0)));
    rig.select(1);

    assert!(!rig.supplied().select(2), "a set of two has no third slot");
    assert_eq!(rig.supplied().selected(), 1);

    rig.present(&filled_with(opaque(0, 255, 0)));

    assert_eq!(
        centre(&rig, &textures[0]),
        [255, 0, 0, 255],
        "a refused slot wrapped round"
    );
    assert_eq!(centre(&rig, &textures[1]), [0, 255, 0, 255]);
}

#[test]
fn a_set_that_cannot_be_presented_into_as_one_is_refused() {
    let _guard = device_lock();
    let graphics = SharedGraphics::new();
    let gpu = open(&graphics);

    assert!(Supplied::new(Vec::new()).is_err(), "an empty set");
    assert!(
        Supplied::new(vec![
            buffer(&gpu, SIDE, FORMAT),
            buffer(&gpu, SIDE, wgpu::TextureFormat::Rgba8Unorm),
        ])
        .is_err(),
        "a set of two formats"
    );
    assert!(
        Supplied::new(vec![
            buffer(&gpu, SIDE, FORMAT),
            buffer(&gpu, SIDE / 2, FORMAT)
        ])
        .is_err(),
        "a set of two extents"
    );
    let refused: [Alteration; 5] = [
        ("no RENDER_ATTACHMENT", |descriptor| {
            descriptor.usage = wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC;
        }),
        ("mip levels", |descriptor| descriptor.mip_level_count = 2),
        ("multisampled", |descriptor| descriptor.sample_count = 4),
        ("layered", |descriptor| {
            descriptor.size.depth_or_array_layers = 2
        }),
        ("three-dimensional", |descriptor| {
            descriptor.dimension = wgpu::TextureDimension::D3;
            descriptor.size.depth_or_array_layers = 2;
        }),
    ];
    for (named, alter) in refused {
        let odd = described(&gpu, alter);
        assert!(Supplied::new(vec![odd]).is_err(), "{named} was accepted");
    }

    let set = Supplied::new(vec![buffer(&gpu, SIDE, FORMAT), buffer(&gpu, SIDE, FORMAT)])
        .expect("two textures that agree are one set");
    assert_eq!(set.len(), 2);
    assert_eq!(set.size(), [SIDE, SIDE], "the extent is the textures' own");
    assert_eq!(set.selected(), 0, "the first is written next");
}

#[test]
fn textures_need_the_device_the_graphics_already_opened() {
    let _guard = device_lock();
    let opened = SharedGraphics::new();
    let textures = vec![buffer(&open(&opened), SIDE, FORMAT)];

    let failure = Supplied::renderer(&SharedGraphics::new(), textures)
        .expect_err("no device is open, so nothing can present into textures made on one");

    assert!(
        failure
            .candidates
            .iter()
            .any(|candidate| candidate.reason.contains("open_gpu")),
        "{failure:?}"
    );
}

#[test]
fn an_encoded_supplied_texture_receives_the_composed_bytes() {
    let (mut rig, _textures) = supplied_in(1, wgpu::TextureFormat::Bgra8UnormSrgb);
    let formats = rig.formats();
    assert_eq!(formats.tier, SrgbTier::UndoInBlit);
    assert_eq!(formats.scene, wgpu::TextureFormat::Bgra8Unorm);
    assert_eq!(
        formats.present_attachment(),
        wgpu::TextureFormat::Bgra8UnormSrgb
    );

    let mut builder = FrameBuilder::new();
    let white = builder.solid(opaque(255, 255, 255));
    // Grey 128 at alpha 128, premultiplied.
    let alpha = 128.0 / 255.0;
    let grey = builder.solid(Color([alpha * alpha, alpha * alpha, alpha * alpha, alpha]));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), white));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), grey));

    let presented = rig.present(&builder.finish());

    let composed = rig.read_composed();
    let blended = composed.rgba(SIDE as i32 / 2, SIDE as i32 / 2);
    assert!(
        blended[0].abs_diff(191) <= 1,
        "the blend happens on encoded values: {blended:?}"
    );
    assert!(
        composed.max_difference(&presented) <= 1,
        "the copy changed the texels by {}",
        composed.max_difference(&presented)
    );
}

#[test]
fn an_output_of_another_extent_stops_frames_until_it_is_configured_back() {
    let (mut rig, textures) = supplied(2);
    rig.select(0);
    rig.present(&filled_with(opaque(255, 0, 0)));
    rig.configure(target(SIDE / 2));
    rig.select(1);

    let outcome = rig.draw(&filled_with(opaque(0, 0, 255)));

    assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Unconfigured));
    assert!(!outcome.retires_damage(), "nothing was recorded");
    assert_eq!(
        centre(&rig, &textures[1]),
        [0, 0, 0, 0],
        "the skipped frame reached a texture"
    );

    rig.configure(target(SIDE));

    rig.present(&filled_with(opaque(0, 0, 255)));

    assert_eq!(centre(&rig, &textures[1]), [0, 0, 255, 255]);
    assert_eq!(
        centre(&rig, &textures[0]),
        [255, 0, 0, 255],
        "the texture on the display lost its frame"
    );
}

#[test]
fn a_supplied_renderer_cannot_rebuild_itself_after_a_loss() {
    let (mut rig, _textures) = supplied(2);
    rig.gpu()
        .loss()
        .report(wgpu::DeviceLostReason::Unknown, "injected by a test");

    assert_eq!(
        rig.draw(&filled_with(opaque(255, 0, 0))),
        FrameOutcome::Skipped(SkipReason::DeviceUnavailable),
        "the renderer did not create these textures and cannot create another set"
    );
}

#[test]
fn each_texture_of_a_rotating_set_receives_its_debt_before_a_frame_lands_on_it() {
    let (mut rig, textures) = supplied(3);
    let steps: [[u8; 3]; 7] = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [0, 255, 255],
        [255, 0, 255],
        [90, 90, 200],
    ];
    let (square, first) = Square::first(opaque(255, 0, 0));
    let mut first = Some(first);
    for (step, [red, green, blue]) in steps.into_iter().enumerate() {
        let slot = (step % textures.len()) as u32;
        rig.select(slot);
        let frame = match first.take() {
            Some(first) => first,
            None => square.next(opaque(red, green, blue), SQUARE, square_damage()),
        };
        let outcome = rig.draw(&frame);
        let stats = outcome
            .stats()
            .unwrap_or_else(|| panic!("step {step}: {outcome:?}"));
        if step > 0 {
            assert_eq!(stats.damage_px, 32 * 32, "step {step} is a partial redraw");
        }
        let pixels = rig.read(&textures[slot as usize]);
        assert_eq!(pixels.rgba(64, 64), [red, green, blue, 255], "step {step}");
        assert_eq!(
            pixels.rgba(2, 2),
            [20, 20, 20, 255],
            "step {step}: texture {slot} never received the background"
        );
    }
}

#[test]
fn a_held_frame_is_owed_to_the_copy_that_follows_it() {
    let (mut rig, textures) = supplied(3);
    let open = Arc::new(AtomicBool::new(true));
    let gate = Arc::clone(&open);
    rig.set_present_gate(Box::new(move || gate.load(Ordering::Relaxed)));
    let left = [8.0, 8.0, 16.0, 16.0];
    let (square, first) = Square::first(opaque(0, 0, 0));
    whole_into_each(&mut rig, textures.len(), &square, first);

    open.store(false, Ordering::Relaxed);
    rig.select(0);
    // The held frame paints a square on the left.
    let held = {
        let mut builder = FrameBuilder::next().damage(area(8, 8, 16, 16));
        builder.set_paint(square.front.index, FramePaint::Solid(opaque(255, 0, 0)));
        builder.quad(filled(
            rect(0.0, 0.0, SIDE as f32, SIDE as f32),
            square.back,
        ));
        builder.quad(filled(left, square.front));
        builder.finish()
    };
    assert_eq!(rig.draw(&held), FrameOutcome::Skipped(SkipReason::Held));

    open.store(true, Ordering::Relaxed);
    let right = area(100, 100, 16, 16);
    for slot in 0..textures.len() as u32 {
        rig.select(slot);
        let mut builder = FrameBuilder::next().damage(right);
        builder.quad(filled(
            rect(0.0, 0.0, SIDE as f32, SIDE as f32),
            square.back,
        ));
        builder.quad(filled(left, square.front));
        builder.quad(filled(rect(100.0, 100.0, 16.0, 16.0), square.front));
        assert!(rig.draw(&builder.finish()).stats().is_some());
        rig.assert_holds_composed(&textures[slot as usize], &format!("slot {slot}"));
    }

    assert_eq!(rig.read(&textures[0]).rgba(16, 16), [255, 0, 0, 255]);
}

#[test]
fn a_held_frame_presented_later_copies_only_what_it_owes() {
    let (mut rig, textures) = supplied(2);
    let (square, first) = Square::first(opaque(0, 0, 0));
    whole_into_each(&mut rig, textures.len(), &square, first);
    // Both images are written whole once, so each owes only what was composed after its copy.
    rig.select(0);
    assert!(
        rig.draw(&square.next(opaque(0, 0, 0), SQUARE, square_damage()))
            .stats()
            .is_some()
    );
    rig.select(1);
    assert!(
        rig.draw(&square.next(opaque(0, 0, 0), SQUARE, square_damage()))
            .stats()
            .is_some()
    );

    rig.set_present_gate(Box::new(|| false));
    rig.select(0);
    let outcome = rig.draw(&square.next(opaque(0, 0, 255), SQUARE, square_damage()));
    assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Held));
    rig.set_present_gate(Box::new(|| true));

    let stats = rig
        .present_held()
        .stats()
        .expect("the held frame is presented");

    assert_eq!(stats.draw_calls, 1, "one owed rectangle, one draw");
    assert_eq!(stats.present_copy_px, 32 * 32);
    rig.assert_holds_composed(&textures[0], "the held frame");

    rig.select(1);
    assert!(
        rig.draw(&square.next(opaque(0, 0, 255), SQUARE, area(100, 100, 4, 4)))
            .stats()
            .is_some()
    );
    rig.assert_holds_composed(&textures[1], "the texture that missed the held frame");
}

#[test]
fn a_shifted_frame_owes_every_texture_the_pixels_it_moved() {
    let (mut rig, textures) = supplied(3);
    let bands = SIDE as usize / 4;
    let mut builder = FrameBuilder::new();
    let paints: Vec<PaintRef> = (0..bands + 8)
        .map(|row| {
            builder.solid(opaque(
                (row * 37 % 256) as u8,
                (row * 91 % 256) as u8,
                (row * 13 % 256) as u8,
            ))
        })
        .collect();
    let stripes = |builder: &mut FrameBuilder, scrolled: usize| {
        for band in 0..bands {
            let y = (band * 4) as f32;
            builder.quad(filled(
                rect(0.0, y, SIDE as f32, 4.0),
                paints[band + scrolled],
            ));
        }
    };
    stripes(&mut builder, 0);
    rig.select(0);
    assert!(rig.draw(&builder.finish()).stats().is_some());
    for slot in 1..textures.len() as u32 {
        rig.select(slot);
        let mut whole = FrameBuilder::next();
        stripes(&mut whole, 0);
        assert!(rig.draw(&whole.finish()).stats().is_some());
    }

    let region = Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32));
    for step in 1..=6 {
        let slot = (step % textures.len()) as u32;
        rig.select(slot);
        let mut next = FrameBuilder::next().damage(area(0, SIDE as i32 - 4, SIDE as i32, 4));
        // The present damage holds the shift's destination as well as the exposed band.
        next.frame_mut().damage.present = DamageSet::full();
        next.frame_mut().shifts.push(Shift {
            region,
            by: [0, -4],
        });
        stripes(&mut next, step);
        let outcome = rig.draw(&next.finish());
        assert!(outcome.stats().is_some(), "step {step}: {outcome:?}");
        rig.assert_holds_composed(&textures[slot as usize], &format!("scroll step {step}"));
    }
}

#[test]
fn present_damage_covers_every_frame_since_the_last_present() {
    let (mut rig, _textures) = supplied(3);
    let open = Arc::new(AtomicBool::new(true));
    let gate = Arc::clone(&open);
    rig.set_present_gate(Box::new(move || gate.load(Ordering::Relaxed)));
    let (square, first) = Square::first(opaque(0, 0, 0));
    rig.select(0);
    assert!(rig.draw(&first).stats().is_some());
    assert!(
        rig.presented_damage().is_full(),
        "the first frame changes everything"
    );

    open.store(false, Ordering::Relaxed);
    rig.select(1);
    let left = square.next(
        opaque(255, 0, 0),
        [8.0, 8.0, 16.0, 16.0],
        area(8, 8, 16, 16),
    );
    assert_eq!(rig.draw(&left), FrameOutcome::Skipped(SkipReason::Held));
    open.store(true, Ordering::Relaxed);
    let right = square.next(
        opaque(255, 0, 0),
        [100.0, 100.0, 16.0, 16.0],
        area(100, 100, 16, 16),
    );
    assert!(rig.draw(&right).stats().is_some());

    let covered = |rig: &Rig, x: i32, y: i32| {
        rig.presented_damage()
            .rects()
            .iter()
            .any(|rect| rect.contains(Point::new(x, y)))
    };
    assert!(covered(&rig, 10, 10), "the held frame's rectangle");
    assert!(covered(&rig, 110, 110), "this frame's rectangle");
    assert!(!covered(&rig, 64, 64), "nothing else changed");
}

#[test]
fn a_consumed_set_owes_only_what_was_composed_since_the_last_copy() {
    let (mut rig, _textures) = supplied(2);
    rig.supplied().set_consumed(true);
    let (square, first) = Square::first(opaque(0, 0, 0));
    rig.select(0);
    assert!(rig.draw(&first).stats().is_some());
    rig.select(1);

    let stats = rig
        .draw(&square.next(opaque(255, 0, 0), SQUARE, square_damage()))
        .stats()
        .expect("presented");

    assert_eq!(
        stats.present_copy_px,
        32 * 32,
        "a consumed texture is copied this frame's square and nothing it missed"
    );
}
