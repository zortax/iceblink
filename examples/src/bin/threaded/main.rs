//! A long list that the compositor scrolls on its own thread, under a bar that slides for ever.
//!
//! The compositor, the renderer and the presenter run on a driver thread of
//! `iceblink_driver::thread`. Wheel and touchpad gestures go to `scroll_input`, so a fling and the
//! sliding bar keep moving while this thread sleeps. The client commits again only when the list
//! comes to rest, to show the row it rests on. A click prints the row under the pointer, found by a
//! hit test, and a key press recolours the square at the header's right end.
//!
//! Every rest and click prints one line to standard output. With `ICEBLINK_FRAME_LOG` set to `1`,
//! every frame also prints one line of statistics to standard error.

mod app;
mod gesture;
mod list;
mod logged;

fn main() {
    app::run();
}
