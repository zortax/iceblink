//! A list the compositor scrolls while its client changes it on every frame.
//!
//! The scene is an opaque backing, a kinetic port that bounces, rows of quads through the port's
//! clip and a thumb beside the port. Rows change colour through a shared paint, fade through an
//! opacity group and are recorded again with inline colours, on frames during wheel glides,
//! touchpad gestures and flings. Some commits acknowledge the newest offset as well.

mod input;
mod list;

pub(crate) use self::input::{drags, glides};
pub(crate) use self::list::{Cadence, ROWS};

use self::input::{Turn, hand};
use self::list::{change, list};
use crate::conformance::end_to_end::drive::Drive;
use crate::conformance::end_to_end::{INCREMENTAL, Pair};
use crate::golden::difference;

/// The refresh interval of the frames drawn.
const FRAME_NS: u64 = 16_666_667;

/// Plays the scene on a drive drawing incrementally and one redrawing whole, with one of `turns`
/// per frame and the client's changes at `cadence`, compares their images after every frame, and
/// returns how many incremental frames shifted pixels.
///
/// Returns zero without drawing when the compositor does not take scroll input or the target
/// factory has no target.
#[track_caller]
pub(crate) fn play(pair: Pair<'_>, turns: Vec<Turn>, cadence: Cadence) -> u32 {
    let (Some(mut kept), Some(mut whole)) = (Drive::new(pair, false), Drive::new(pair, true))
    else {
        return 0;
    };
    if !kept.compositor().capabilities().scroll_input {
        eprintln!("skipped: the compositor takes no scroll input");
        return 0;
    }

    let lists = [&mut kept, &mut whole].map(|drive| {
        let output = drive.output();
        let mut built = None;
        drive.commit(|tx| built = Some(list(tx, output)));
        built.expect("the scene was built")
    });
    for drive in [&mut kept, &mut whole] {
        drive.at(0, FRAME_NS);
        drive.draw();
    }

    let mut shifted = 0;
    for (frame, turn, present_ns) in turns
        .into_iter()
        .zip(1..)
        .map(|(turn, frame)| (frame, turn, u64::from(frame) * FRAME_NS))
    {
        for (drive, list) in [&mut kept, &mut whole].into_iter().zip(lists) {
            hand(drive, turn.before, present_ns - FRAME_NS);
            let events = drive.events();
            drive.commit(|tx| change(tx, list, frame, cadence, &events));
            hand(drive, turn.after, present_ns - FRAME_NS / 2);
            drive.at(present_ns, FRAME_NS);
        }
        let left = kept.draw();
        if kept.compositor().counters().frame.shifted_px > 0 {
            shifted += 1;
        }
        let right = whole.draw();
        let found = difference(&left, &right, INCREMENTAL).expect("the images have one size");
        assert!(
            found.pixels == 0,
            "frame {frame} of the driven list differs from the whole redraw in {} pixels, the \
             first at {:?}, by up to {}",
            found.pixels,
            found.first,
            found.max_channel
        );
    }
    shifted
}
