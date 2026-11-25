//! The frame and commit cost of these scenes:
//!
//! - idle frames, and a one-chunk change;
//! - scrolling long lists, a virtual list whose rows are replaced, reused or all moved, and a
//!   page;
//! - a compositor fling over a long list;
//! - dragging the edge between two clipped panes;
//! - a window dragged wider over a conversation, alone and beside panels recorded again at every
//!   width;
//! - tables of 30, 300 and 3000 rows that a search or a filter hides and shows again;
//! - a first frame full of text.

mod drag;
mod fling;
mod items;
mod layout;
mod list;
mod page;
mod recycle;
mod resize;
mod scroll;
mod table;

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use iceblink::Compositor;
use iceblink::{Color, Paint};

use self::drag::Drag;
use self::fling::Fling;
use self::list::{List, Rows};
use self::page::Page;
use self::recycle::{Departing, Recycler};
use self::resize::Resize;
use self::scroll::Scroller;
use self::table::{Change, Table};

/// A list of 1000 rows with a zebra quad on every other row.
const LONG: Rows = Rows {
    count: 1000,
    quad_every: 2,
};

/// A list of 10,000 rows with a background quad on every row.
const TALL: Rows = Rows {
    count: 10_000,
    quad_every: 1,
};

fn benches(c: &mut Criterion) {
    let mut list = List::new(LONG);
    c.bench_function("frames/idle", |b| {
        b.iter(|| black_box(list.draw()));
    });

    let mut list = List::new(LONG);
    let mut blue = 0.0;
    c.bench_function("frames/one_chunk_change", |b| {
        b.iter(|| {
            blue = 1.0 - blue;
            let mut tx = list.compositor.transaction(list.client);
            tx.set_paint(list.marked, Paint::Solid(Color([0.1, 0.2, blue, 1.0])));
            list.compositor.commit(tx).expect("the paint commits");
            black_box(list.draw())
        });
    });

    let mut scroller = Scroller::new(LONG);
    scroller.report("frames/scroll_long_list");
    c.bench_function("frames/scroll_long_list", |b| {
        b.iter(|| {
            scroller.commit();
            black_box(scroller.draw())
        });
    });

    let mut scroller = Scroller::new(TALL);
    scroller.report("frames/scroll_10k");
    c.bench_function("frames/scroll_10k/commit", |b| {
        b.iter_custom(|iterations| scroller.time_commits(iterations));
    });
    c.bench_function("frames/scroll_10k/frame", |b| {
        b.iter_custom(|iterations| scroller.time_frames(iterations));
    });

    let mut fling = Fling::new(TALL);
    fling.report("frames/fling_10k");
    c.bench_function("frames/fling_10k/tick", |b| {
        b.iter_custom(|iterations| fling.time_ticks(iterations));
    });

    let mut recycler = Recycler::new(Departing::Replaced);
    recycler.report("frames/recycle");
    c.bench_function("frames/recycle/commit", |b| {
        b.iter_custom(|iterations| recycler.time_commits(iterations));
    });
    c.bench_function("frames/recycle/frame", |b| {
        b.iter_custom(|iterations| recycler.time_frames(iterations));
    });

    let mut reuser = Recycler::new(Departing::Reused);
    reuser.report("frames/reuse");
    c.bench_function("frames/reuse/commit", |b| {
        b.iter_custom(|iterations| reuser.time_commits(iterations));
    });
    c.bench_function("frames/reuse/frame", |b| {
        b.iter_custom(|iterations| reuser.time_frames(iterations));
    });

    let mut shifter = Recycler::new(Departing::Shifted);
    shifter.report("frames/shift");
    c.bench_function("frames/shift/commit", |b| {
        b.iter_custom(|iterations| shifter.time_commits(iterations));
    });
    c.bench_function("frames/shift/frame", |b| {
        b.iter_custom(|iterations| shifter.time_frames(iterations));
    });

    let mut page = Page::new();
    page.report("frames/scroll_page");
    c.bench_function("frames/scroll_page/commit", |b| {
        b.iter_custom(|iterations| page.time_commits(iterations));
    });
    c.bench_function("frames/scroll_page/frame", |b| {
        b.iter_custom(|iterations| page.time_frames(iterations));
    });

    let mut drag = Drag::new();
    drag.report("frames/clip_drag");
    c.bench_function("frames/clip_drag/commit", |b| {
        b.iter_custom(|iterations| drag.time_commits(iterations));
    });
    c.bench_function("frames/clip_drag/frame", |b| {
        b.iter_custom(|iterations| drag.time_frames(iterations));
    });

    let mut resize = Resize::new();
    resize.report("frames/resize");
    c.bench_function("frames/resize/commit", |b| {
        b.iter_custom(|iterations| resize.time_commits(iterations));
    });
    c.bench_function("frames/resize/frame", |b| {
        b.iter_custom(|iterations| resize.time_frames(iterations));
    });

    let mut panels = Resize::with_panels();
    panels.report("frames/resize_panels");
    c.bench_function("frames/resize_panels/commit", |b| {
        b.iter_custom(|iterations| panels.time_commits(iterations));
    });
    c.bench_function("frames/resize_panels/frame", |b| {
        b.iter_custom(|iterations| panels.time_frames(iterations));
    });

    for (rows, name) in [(30, "table_30"), (300, "table"), (3000, "table_3000")] {
        let mut search = Table::new(Change::Search, rows);
        search.report(&format!("frames/{name}_search"));
        c.bench_function(&format!("frames/{name}_search/hide"), |b| {
            b.iter_custom(|iterations| search.time_commits(iterations, true));
        });
        c.bench_function(&format!("frames/{name}_search/show"), |b| {
            b.iter_custom(|iterations| search.time_commits(iterations, false));
        });
        c.bench_function(&format!("frames/{name}_search/frame"), |b| {
            b.iter_custom(|iterations| search.time_frames(iterations));
        });
        c.bench_function(&format!("frames/{name}_search/blink"), |b| {
            b.iter_custom(|iterations| search.time_blinks(iterations));
        });

        let mut filter = Table::new(Change::Filter, rows);
        filter.report(&format!("frames/{name}_filter"));
        c.bench_function(&format!("frames/{name}_filter/hide"), |b| {
            b.iter_custom(|iterations| filter.time_commits(iterations, true));
        });
        c.bench_function(&format!("frames/{name}_filter/show"), |b| {
            b.iter_custom(|iterations| filter.time_commits(iterations, false));
        });
    }

    c.bench_function("frames/text_first_frame", |b| {
        b.iter_batched(
            || List::committed(LONG),
            |mut list| black_box(list.draw()),
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(frames, benches);
criterion_main!(frames);
