//! Hit tests answer from what the most recent frame shows, also while a fling moves the list.

mod common;

use iceblink::smallvec::smallvec;
use iceblink::{ClientEvent, CompositorClient, OffsetRange, ScrollWatch, WatchCause};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, ROW, ROWS, scrolls, spawn};
use crate::common::thread::wake::Wakes;
use crate::common::thread::watched::Watched;

/// The point the hit tests probe, in the output's pixels.
// Off the whole pixels, so the point never lies on the edge between two rows.
const POINT: [f32; 2] = [100.0, 10.5];

/// Returns the row of the list under `POINT` when the list shows `offset`.
fn row_at(offset: f32) -> u32 {
    ((POINT[1] + offset) / ROW) as u32
}

#[test]
fn hit_tests_during_a_fling_find_the_rows_the_frames_show() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let wakes = Wakes::of(&mut handle, list.client);
    let mut events = Vec::new();
    handle.drain_events(list.client, &mut events);
    events.clear();
    list.fling(&mut handle, 2000.0);

    let newest = |events: &[ClientEvent]| {
        scrolls(events, list.node)
            .last()
            .map_or(0.0, |(_, offset, _)| *offset)
    };
    let mut rows = Vec::new();
    let mut shown = 0.0;
    loop {
        // Every row the list moves past wakes the client.
        let watch = ScrollWatch {
            ranges: smallvec![OffsetRange {
                bounds: [[0.0, 0.0], [shown, shown + ROW]],
                ahead: 0.0,
            }],
            settle: true,
            ..ScrollWatch::default()
        };
        handle.watch_scroll(list.client, list.node, Some(watch));
        handle.drain_events(list.client, &mut events);
        let before = newest(&events);
        let hits = handle.hit_test(list.output, POINT);
        handle.drain_events(list.client, &mut events);
        let after = newest(&events);

        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].chunk, ROWS);
        let row = hits[0].item;
        assert!(
            (row_at(before)..=row_at(after)).contains(&row),
            "row {row} lies between the offsets {before} and {after}"
        );
        rows.push(row);
        shown = after;

        let settled = events.iter().any(|event| {
            matches!(
                event,
                ClientEvent::Watched {
                    cause: WatchCause::Settled,
                    ..
                }
            )
        });
        if settled {
            break;
        }
        wakes.next();
    }

    assert!(rows.windows(2).all(|pair| pair[0] <= pair[1]), "{rows:?}");
    rows.dedup();
    assert!(rows.len() > 5, "the fling moved under the point: {rows:?}");
    driver.shutdown();
}
