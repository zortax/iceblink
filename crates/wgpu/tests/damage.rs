//! Partial redraws: only the damage is drawn, shifts move kept pixels, and opaque covers skip
//! what they hide.

mod common;

use common::build::{FrameBuilder, area, filled, frosted, group, opaque, rect, straight, whole};
use common::device::{SIDE, TestRenderer, draw, pair, plain_renderer, present, tolerance};
use iceblink::Renderer as _;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    BufferAge, ClipRef, DeviceRect, FRAME_NO_CLIP, Filter, Frame, Line, LineStyle, Matrix4,
    OpaqueCover, Shift, SpaceRef,
};

#[test]
fn a_partial_frame_keeps_every_pixel_outside_its_damage() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let red = first.solid(opaque(255, 0, 0));
    let blue = first.solid(opaque(0, 0, 255));
    first.quad(filled(rect(0.0, 0.0, 128.0, 128.0), red));
    present(&mut renderer, &first.finish());
    assert_eq!(renderer.target_age(), BufferAge::Kept);

    let mut next = FrameBuilder::next().damage(area(0, 0, 32, 32));
    next.quad(filled(rect(0.0, 0.0, 128.0, 128.0), blue));

    let stats = draw(&mut renderer, &next.finish()).stats().unwrap();

    assert_eq!(stats.damage_px, 32 * 32);
    let pixels = renderer.read_presented().unwrap();
    assert_eq!(pixels.rgba(16, 16), [0, 0, 255, 255], "inside the damage");
    assert_eq!(pixels.rgba(64, 64), [255, 0, 0, 255], "outside it");
}

#[test]
fn a_shift_moves_kept_pixels_before_the_damage_is_drawn() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let green = first.solid(opaque(0, 255, 0));
    first.quad(filled(rect(0.0, 0.0, 128.0, 16.0), green));
    present(&mut renderer, &first.finish());

    let region = Rect::new(Point::new(0, 0), Size::new(128, 128));
    let mut next = FrameBuilder::next().damage(area(0, 0, 128, 32));
    // Present damage holds the shift's destination as well as the drawn band.
    next.frame_mut().damage.present = area(0, 0, 128, 128);
    next.frame_mut().shifts.push(Shift {
        region,
        by: [0, 32],
    });

    let pixels = present(&mut renderer, &next.finish());

    assert_eq!(
        pixels.rgba(64, 40),
        [0, 255, 0, 255],
        "the band moved down by 32"
    );
    assert_eq!(
        pixels.rgba(64, 8),
        [0, 0, 0, 0],
        "the exposed band was redrawn empty"
    );
}

#[test]
fn an_opaque_cover_starts_the_redraw_at_its_quad() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let red = first.solid(opaque(255, 0, 0));
    let white = first.solid(opaque(255, 255, 255));
    first.quad(filled(rect(0.0, 0.0, 128.0, 128.0), red));
    present(&mut renderer, &first.finish());

    let mut next = FrameBuilder::next().damage(area(8, 8, 16, 16));
    // The red quad sits beneath the cover and is skipped, so only the white one draws.
    next.quad(filled(rect(0.0, 0.0, 128.0, 128.0), red));
    let mut cover = filled(rect(0.0, 0.0, 64.0, 64.0), white);
    cover.fill = white;
    next.quad(cover);
    let mut frame = next.finish();
    frame.opaque_covers.push(OpaqueCover {
        rect: Rect::new(Point::new(1, 1), Size::new(62, 62)),
        batch: 0,
        at: 1,
    });

    let stats = draw(&mut renderer, &frame).stats().unwrap();

    let pixels = renderer.read_presented().unwrap();
    assert_eq!(pixels.rgba(16, 16), [255, 255, 255, 255]);
    assert_eq!(
        stats.draw_calls, 2,
        "no clear, one batch from the cover's quad, and the copy"
    );
}

/// An item of a test frame: an opaque quad, moved by a translating space when `shift` is set, or
/// a blurred backdrop.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Item {
    bounds: [f32; 4],
    color: [u8; 3],
    frost: Option<f32>,
    shift: [f32; 2],
}

impl Item {
    /// Returns an opaque quad over `bounds` in `color`.
    fn plain(bounds: [f32; 4], color: [u8; 3]) -> Self {
        Self {
            bounds,
            color,
            frost: None,
            shift: [0.0; 2],
        }
    }

    /// Returns the item moved by a translating space of `shift`.
    fn placed(mut self, shift: [f32; 2]) -> Self {
        self.shift = shift;
        self
    }

    /// Returns a backdrop over `bounds` that blurs by `deviation`.
    fn frosted(bounds: [f32; 4], deviation: f32) -> Self {
        Self {
            bounds,
            color: [0; 3],
            frost: Some(deviation),
            shift: [0.0; 2],
        }
    }

    /// Returns the bounds the item covers once it is moved.
    fn ink(&self) -> [f32; 4] {
        let [x, y, width, height] = self.bounds;
        [x + self.shift[0], y + self.shift[1], width, height]
    }

    /// Returns the bounds the item reads.
    fn source(&self) -> [f32; 4] {
        match self.frost {
            Some(deviation) => frosted(self.ink(), deviation).source,
            None => self.ink(),
        }
    }
}

/// Returns a white background with `items` over it, drawn over `damage`.
fn build(items: &[Item], damage: DamageSet) -> Frame {
    let mut builder = FrameBuilder::new().damage(damage);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), white));
    for item in items {
        if let Some(deviation) = item.frost {
            builder.backdrop(frosted(item.ink(), deviation));
            continue;
        }
        let paint = builder.solid(opaque(item.color[0], item.color[1], item.color[2]));
        let mut quad = filled(item.bounds, paint);
        if item.shift != [0.0; 2] {
            let space = builder.space(Matrix4::translation(item.shift[0], item.shift[1], 0.0));
            quad.space = SpaceRef(space);
        }
        builder.quad(quad);
    }
    builder.finish()
}

/// Returns the whole pixels `bounds` touches, grown by one and cut to the output.
fn pixels_of(bounds: [f32; 4]) -> DeviceRect {
    let [x, y, width, height] = bounds;
    Rect::from_corners(
        Point::new(x.floor() as i32 - 1, y.floor() as i32 - 1),
        Point::new(
            (x + width).ceil() as i32 + 1,
            (y + height).ceil() as i32 + 1,
        ),
    )
    .intersection(whole())
    .unwrap_or(Rect::ZERO)
}

/// Returns the damage between `before` and `after`, closed over what backdrops read, as a
/// compositor derives it.
fn damage_between(before: &[Item], after: &[Item]) -> DamageSet {
    let mut damage = DamageSet::new();
    let common = before.len().min(after.len());
    for index in 0..common {
        if before[index] != after[index] {
            damage.absorb(pixels_of(before[index].ink()));
            damage.absorb(pixels_of(after[index].ink()));
        }
    }
    for item in before.iter().skip(common).chain(after.iter().skip(common)) {
        damage.absorb(pixels_of(item.ink()));
    }
    loop {
        let before_closure = damage;
        for item in after.iter().filter(|item| item.frost.is_some()) {
            if damage.intersects(pixels_of(item.ink())) {
                damage.absorb(pixels_of(item.source()));
            }
        }
        if damage == before_closure {
            return damage;
        }
    }
}

/// Draws the change from `previous` to `items` over its damage and asserts that it is the frame
/// drawn whole.
fn assert_redrawn(
    tracked: &mut TestRenderer,
    whole: &mut TestRenderer,
    previous: &[Item],
    items: &[Item],
    what: &str,
) {
    let damage = damage_between(previous, items);
    let damaged = present(tracked, &build(items, damage));
    let full = present(whole, &build(items, DamageSet::full()));
    assert_eq!(
        damaged.max_difference(&full),
        0,
        "{what}: redrawing {:?} did not draw the whole frame",
        damage.rects()
    );
}

/// Draws `items` whole on both renderers.
fn start(tracked: &mut TestRenderer, whole: &mut TestRenderer, items: &[Item]) {
    present(tracked, &build(items, DamageSet::full()));
    present(whole, &build(items, DamageSet::full()));
}

/// A linear congruential generator, so every run builds the same frames.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.0 >> 33
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }

    fn coordinate(&mut self) -> f32 {
        (self.next() % 100) as f32
    }
}

/// Returns three overlapping opaque quads.
fn base() -> Vec<Item> {
    vec![
        Item::plain(rect(8.0, 8.0, 48.0, 48.0), [200, 40, 40]),
        Item::plain(rect(40.0, 24.0, 56.0, 40.0), [40, 200, 40]),
        Item::plain(rect(16.0, 72.0, 96.0, 32.0), [40, 40, 200]),
    ]
}

#[test]
fn a_frame_drawn_over_its_damage_is_the_frame_drawn_whole() {
    let (mut tracked, mut whole) = pair();
    let mut rng = Rng(0x5eed);
    for seed in 0..24u32 {
        let mut items = base();
        start(&mut tracked, &mut whole, &items);
        for step in 0..6 {
            let previous = items.clone();
            match (rng.next() % 4, items.len()) {
                (0, _) => {
                    let at = rng.below(items.len() + 1);
                    let color = [(seed * 40) as u8, (step * 40) as u8, 90];
                    let bounds = rect(rng.coordinate(), rng.coordinate(), 24.0, 24.0);
                    items.insert(at, Item::plain(bounds, color));
                }
                (1, count) if count > 1 => {
                    let at = rng.below(count);
                    items.remove(at);
                }
                (2, count) if count > 0 => {
                    let at = rng.below(count);
                    items[at].bounds[0] = rng.coordinate();
                    items[at].bounds[1] = rng.coordinate();
                }
                (_, count) if count > 0 => {
                    let at = rng.below(count);
                    items[at].color = [(rng.next() % 256) as u8, 30, 30];
                }
                _ => continue,
            }
            let what = format!("seed {seed} step {step}");
            assert_redrawn(&mut tracked, &mut whole, &previous, &items, &what);
        }
    }
}

/// Returns a backdrop blurred by `deviation` over a quad that has moved `step` times.
fn frosted_over_moving_content(step: i32, deviation: f32) -> Vec<Item> {
    vec![
        Item::plain(rect(0.0, 0.0, 128.0, 128.0), [230, 230, 230]),
        Item::plain(
            rect(8.0 + step as f32 * 3.0, 40.0, 32.0, 48.0),
            [20, 20, 200],
        ),
        Item::frosted(rect(32.0, 32.0, 64.0, 64.0), deviation),
    ]
}

// A small or zero blur reads only a pixel or two past its bounds, where a seam would show.
#[test]
fn a_backdrop_over_moving_content_is_redrawn_without_smearing_or_a_seam() {
    for deviation in [5.0, 0.2, 0.0] {
        let (mut tracked, mut whole) = pair();
        let mut items = frosted_over_moving_content(0, deviation);
        start(&mut tracked, &mut whole, &items);
        for step in 1..8 {
            let previous = items.clone();
            items = frosted_over_moving_content(step, deviation);
            let what = format!("blur({deviation}) step {step}");
            assert_redrawn(&mut tracked, &mut whole, &previous, &items, &what);
        }

        let last = damage_between(
            &frosted_over_moving_content(6, deviation),
            &frosted_over_moving_content(7, deviation),
        );
        assert!(!last.is_full(), "the fixture redraws part of the output");
    }
}

/// Returns opaque quads, one of them in a space that has moved `step` times.
fn moving_space(step: i32) -> Vec<Item> {
    vec![
        Item::plain(rect(0.0, 0.0, 128.0, 128.0), [235, 235, 235]),
        Item::plain(rect(20.0, 20.0, 40.0, 40.0), [200, 60, 60]),
        Item::plain(rect(10.0, 60.0, 36.0, 28.0), [30, 90, 210]).placed([step as f32 * 7.0, 0.0]),
        Item::plain(rect(70.0, 70.0, 30.0, 30.0), [40, 170, 90]),
    ]
}

#[test]
fn a_quad_in_a_moving_space_is_redrawn_to_the_frame_drawn_whole() {
    let (mut tracked, mut whole) = pair();
    let mut items = moving_space(0);
    start(&mut tracked, &mut whole, &items);
    for step in 1..10 {
        let previous = items.clone();
        items = moving_space(step);
        let what = format!("step {step}");
        assert_redrawn(&mut tracked, &mut whole, &previous, &items, &what);
    }
}

/// Returns overlapping opaque quads, a panel that covers most of them, and a translucent square at
/// `x` over the panel, with the panel as an opaque cover.
fn covered(x: f32, damage: DamageSet) -> Frame {
    let mut builder = FrameBuilder::new().damage(damage);
    for (bounds, color) in [
        (rect(0.0, 0.0, 128.0, 128.0), opaque(255, 255, 255)),
        (rect(10.0, 10.0, 60.0, 30.0), opaque(200, 30, 30)),
        (rect(40.0, 30.0, 60.0, 50.0), opaque(30, 160, 30)),
        (rect(20.0, 70.0, 80.0, 40.0), opaque(240, 200, 20)),
        (rect(16.0, 16.0, 96.0, 96.0), opaque(20, 40, 160)),
        (rect(x, 48.0, 20.0, 20.0), straight(255, 255, 255, 128)),
    ] {
        let paint = builder.solid(color);
        builder.quad(filled(bounds, paint));
    }
    let mut frame = builder.finish();
    frame.opaque_covers.push(OpaqueCover {
        rect: Rect::new(Point::new(16, 16), Size::new(96, 96)),
        batch: 0,
        at: 4,
    });
    frame
}

#[test]
fn a_rectangle_under_an_opaque_cover_is_redrawn_to_the_frame_drawn_whole() {
    let (mut tracked, mut whole) = pair();
    present(&mut tracked, &covered(30.0, DamageSet::full()));
    for step in 1..6 {
        let (was, is) = (30.0 + 6.0 * (step - 1) as f32, 30.0 + 6.0 * step as f32);
        let mut damage = DamageSet::new();
        damage.absorb(pixels_of(rect(was, 48.0, 20.0, 20.0)));
        damage.absorb(pixels_of(rect(is, 48.0, 20.0, 20.0)));
        let damaged = present(&mut tracked, &covered(is, damage));
        let full = present(&mut whole, &covered(is, DamageSet::full()));
        assert_eq!(
            damaged.max_difference(&full),
            0,
            "step {step}: the rectangle under the cover differs from the frame drawn whole"
        );
    }
}

/// Returns the damage of the top and bottom halves of the output, as two rectangles.
fn halves() -> DamageSet {
    let mut halves = area(0, 0, 128, 64);
    halves.absorb_set(&area(0, 64, 128, 64));
    assert_eq!(halves.len(), 2, "the two halves are disjoint rectangles");
    halves
}

/// Returns a half-transparent black group over white, inside a group blurred by `outer` when set.
fn grouped(outer: Option<f32>, damage: DamageSet) -> Frame {
    let mut builder = FrameBuilder::new().damage(damage);
    let white = builder.solid(opaque(255, 255, 255));
    let black = builder.solid(opaque(0, 0, 0));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), white));
    if let Some(deviation) = outer {
        builder.begin_group(group(
            rect(16.0, 16.0, 96.0, 96.0),
            1.0,
            &[Filter::Blur(deviation)],
        ));
    }
    builder.begin_group(group(rect(24.0, 24.0, 80.0, 80.0), 0.5, &[]));
    builder.quad(filled(rect(24.0, 24.0, 80.0, 80.0), black));
    builder.end_group();
    if outer.is_some() {
        builder.end_group();
    }
    builder.finish()
}

// A blur reads past the rectangle it is drawn in, so the group inside it is composited over all of
// what the blur reads.
#[test]
fn a_group_drawn_in_two_rectangles_is_the_group_drawn_in_one() {
    for outer in [None, Some(4.0)] {
        let (mut split, mut once) = pair();
        present(&mut split, &grouped(outer, DamageSet::full()));
        let in_halves = present(&mut split, &grouped(outer, halves()));
        let in_one = present(&mut once, &grouped(outer, DamageSet::full()));
        assert_eq!(
            in_halves.max_difference(&in_one),
            0,
            "blurred by {outer:?}: the group differs when its rectangle is split"
        );
    }
}

/// Returns stacks of lines and quads in the top left and, turned, in the bottom right, and a
/// blurred group at the left edge, with the top quad of each stack tinted `tint`.
fn culled_corners(tint: [u8; 3], damage: DamageSet) -> Frame {
    let mut builder = FrameBuilder::new().damage(damage);
    let white = builder.solid(opaque(255, 255, 255));
    let grey = builder.solid(opaque(90, 90, 90));
    let tinted = builder.solid(opaque(tint[0], tint[1], tint[2]));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), white));
    let (sin, cos) = 0.3f32.sin_cos();
    let turned = builder.space(Matrix4::from_columns([
        [cos, sin, 0.0, 0.0],
        [-sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [96.0, 96.0, 0.0, 1.0],
    ]));
    for (origin, space, layers) in [([8.0, 8.0], 0, 5), ([-16.0, -16.0], turned, 6)] {
        for layer in 0..layers {
            let top = layer + 1 == layers;
            let height = if top { 12.0 } else { 8.0 };
            let at = rect(origin[0], origin[1] + 3.0 * layer as f32, 32.0, height);
            if (layer + layers) % 2 == 0 {
                builder.line(line(at, opaque(200, 30, 30), space));
            } else {
                let mut quad = filled(at, if top { tinted } else { grey });
                quad.space = SpaceRef(space);
                builder.quad(quad);
            }
        }
    }
    builder.begin_group(group(
        rect(0.0, 48.0, 64.0, 24.0),
        1.0,
        &[Filter::Blur(3.0)],
    ));
    builder.quad(filled(rect(4.0, 52.0, 16.0, 8.0), tinted));
    builder.line(line(rect(44.0, 52.0, 12.0, 6.0), opaque(30, 30, 200), 0));
    builder.end_group();
    builder.finish()
}

/// Returns a solid line two pixels thick over `bounds` in space `space`.
fn line(bounds: [f32; 4], color: iceblink::Color, space: u32) -> Line {
    Line {
        bounds,
        color,
        thickness: 2.0,
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(space),
        style: LineStyle::SOLID,
        _pad: [0; 3],
    }
}

#[test]
fn a_frame_with_batches_left_out_of_a_rectangle_is_the_frame_drawn_whole() {
    let (mut split, mut once) = pair();
    present(
        &mut split,
        &culled_corners([40, 160, 40], DamageSet::full()),
    );
    let mut corners = area(0, 0, 42, 80);
    corners.absorb_set(&area(68, 68, 56, 56));
    assert_eq!(corners.len(), 2, "two disjoint rectangles");

    let outcome = draw(&mut split, &culled_corners([160, 40, 160], corners));
    let culled = outcome.stats().expect("presented").batches_culled;
    let in_corners = split.read_presented().expect("an offscreen image");
    let whole = present(
        &mut once,
        &culled_corners([160, 40, 160], DamageSet::full()),
    );

    // lavapipe rounds four blurred pixels of the group by one step when the rectangle cuts it.
    assert!(
        in_corners.max_difference(&whole) <= tolerance(0),
        "the corners redrawn with batches left out are the frame drawn whole"
    );
    assert!(culled > 0, "the fixture leaves batches out of a rectangle");
}

#[test]
fn a_frame_reports_the_rectangles_and_rows_it_drew() {
    let mut renderer = plain_renderer();
    let output = Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32));
    let items = base();
    present(&mut renderer, &build(&items, DamageSet::full()));

    assert_eq!(renderer.composed_rects(), [output]);
    assert_eq!(renderer.composed_rows(), std::slice::from_ref(&(0..SIDE)));

    let mut recoloured = items.clone();
    recoloured[2].color = [10, 220, 10];
    let damage = damage_between(&items, &recoloured);

    draw(&mut renderer, &build(&recoloured, damage));

    let reported = renderer.composed_rects().to_vec();
    let covered: i64 = reported
        .iter()
        .map(|rect| i64::from(rect.size.width) * i64::from(rect.size.height))
        .sum();
    assert!(
        !reported.is_empty() && covered < i64::from(SIDE * SIDE),
        "one rectangle was redrawn: {reported:?}"
    );
    assert!(
        reported
            .iter()
            .any(|rect| rect.contains(Point::new(64, 88))),
        "the recoloured quad is in the report: {reported:?}"
    );
    let mut rows: Vec<_> = reported
        .iter()
        .map(|rect| rect.origin.y as u32..(rect.origin.y + rect.size.height) as u32)
        .collect();
    rows.sort_by_key(|band| band.start);
    let mut merged: Vec<std::ops::Range<u32>> = Vec::new();
    for band in rows {
        match merged.last_mut() {
            Some(last) if band.start <= last.end => last.end = last.end.max(band.end),
            _ => merged.push(band),
        }
    }
    assert_eq!(
        renderer.composed_rows(),
        merged,
        "the rows are the rectangles' rows"
    );
}
