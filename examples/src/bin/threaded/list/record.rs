//! Recording of the list's chunks.

use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationStart, AnimationTarget, Direction, Easing, FillMode, Keyframe, Keyframes,
    Matrix4, PaintKind, Timing, Transaction,
};
use iceblink_examples::{Line, quad};

use super::layout::{port, props, rows_key};
use super::{
    BACKGROUND, BAR, BAR_WIDTH, HEADER, List, MARGIN, MARKER, MARKER_SIDE, PER_CHUNK, ROW,
    SLIDE_NS, STATUS, TRACK, TRACK_TOP,
};

impl List {
    /// Records the background and the white face of the port.
    pub(super) fn record_frame(&self, tx: &mut Transaction) {
        let mut chunk = tx.record(BACKGROUND, props(self.space));
        let background = chunk.paint_ref(PaintKind::SOLID, self.paints.background);
        chunk.quad(quad([0.0, 0.0, self.size[0], self.size[1]], background));
        // The port has square corners and one opaque quad behind it, so a scroll moves its pixels.
        let face = chunk.paint_ref(PaintKind::SOLID, self.paints.port);
        chunk.quad(quad(port(self.size), face));
    }

    pub(super) fn record_header(&self, tx: &mut Transaction) {
        let mut chunk = tx.record(HEADER, props(self.space));
        let ink = chunk.paint_ref(PaintKind::SOLID, self.paints.ink);
        self.font.record(
            &mut chunk,
            &Line {
                text: "A thousand rows, scrolled by the compositor's thread",
                size: 22.0,
                origin: [MARGIN, 32.0],
                paint: ink,
            },
        );
    }

    /// Records the square at the header's right end, in the colour `marked` says.
    pub(super) fn record_marker(&self, tx: &mut Transaction) {
        let paint = if self.marked {
            self.paints.bar
        } else {
            self.paints.track
        };
        let mut chunk = tx.record(MARKER, props(self.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let left = self.size[0] - MARGIN - MARKER_SIDE;
        chunk.quad(quad([left, 16.0, MARKER_SIDE, MARKER_SIDE], fill));
    }

    pub(super) fn record_status(&self, tx: &mut Transaction, text: &str) {
        let mut chunk = tx.record(STATUS, props(self.space));
        let ink = chunk.paint_ref(PaintKind::SOLID, self.paints.ink);
        self.font.record(
            &mut chunk,
            &Line {
                text,
                size: 15.0,
                origin: [MARGIN, 56.0],
                paint: ink,
            },
        );
    }

    /// Records the track and the bar that slides along it.
    pub(super) fn record_bar(&self, tx: &mut Transaction) {
        let mut chunk = tx.record(TRACK, props(self.space));
        let track = chunk.paint_ref(PaintKind::SOLID, self.paints.track);
        let mut band = quad([MARGIN, TRACK_TOP, self.track_width(), 6.0], track);
        band.radii = [3.0; 8];
        chunk.quad(band);

        let mut chunk = tx.record(BAR, props(self.bar));
        let fill = chunk.paint_ref(PaintKind::SOLID, self.paints.bar);
        let mut bar = quad([MARGIN, TRACK_TOP, BAR_WIDTH, 6.0], fill);
        bar.radii = [3.0; 8];
        chunk.quad(bar);
    }

    /// Records rows `chunk * PER_CHUNK ..` in the content space, clipped to the port.
    pub(super) fn record_rows(&self, tx: &mut Transaction, chunk: usize) {
        let mut props = props(self.content);
        props.clip = Some(self.port);
        let mut recorder = tx.record(rows_key(chunk), props);
        let even = recorder.paint_ref(PaintKind::SOLID, self.paints.even);
        let odd = recorder.paint_ref(PaintKind::SOLID, self.paints.odd);
        let ink = recorder.paint_ref(PaintKind::SOLID, self.paints.ink);
        let [x, y, width, _] = port(self.size);
        for row in chunk * PER_CHUNK..(chunk + 1) * PER_CHUNK {
            let top = y + 4.0 + row as f32 * ROW;
            let fill = if row % 2 == 0 { even } else { odd };
            let mut band = quad([x + 4.0, top, width - 8.0, ROW - 4.0], fill);
            band.radii = [6.0; 8];
            recorder.quad(band);
            self.font.record(
                &mut recorder,
                &Line {
                    text: &format!("Row {}", row + 1),
                    size: 16.0,
                    origin: [x + 16.0, top + 22.0],
                    paint: ink,
                },
            );
        }
    }

    /// Starts the bar sliding to the end of its track and back, for ever.
    ///
    /// An animation started earlier is replaced.
    pub(super) fn slide(&self, tx: &mut Transaction) {
        let travel = (self.track_width() - BAR_WIDTH).max(0.0);
        let key = |offset, value| Keyframe {
            offset,
            value,
            easing: Easing::Linear,
        };
        tx.animate(Animation {
            target: AnimationTarget::Transform(self.bar),
            keyframes: Keyframes::Matrix(smallvec![
                key(0.0, Matrix4::IDENTITY),
                key(1.0, Matrix4::translation(travel, 0.0, 0.0)),
            ]),
            timing: Timing {
                duration_ns: SLIDE_NS,
                delay_ns: 0,
                iterations: f32::INFINITY,
                direction: Direction::Alternate,
                fill: FillMode::None,
                easing: Easing::CubicBezier([0.42, 0.0, 0.58, 1.0]),
            },
            start: AnimationStart::Next,
        });
    }

    fn track_width(&self) -> f32 {
        (self.size[0] - 2.0 * MARGIN).max(1.0)
    }
}
