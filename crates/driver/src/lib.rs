//! Runs a compositor's frame lifecycle behind the client seam.
//!
//! [`InProcess`] is a [`CompositorClient`](iceblink::CompositorClient) that builds frames on
//! the client's own thread, when the client pumps or steps it. It needs no thread of its own, so
//! it also runs on the web. [`thread::ThreadedCompositor`] builds them on a driver thread instead.
//!
//! [`Driver`] and [`Mailbox`] are the parts both drivers are built from. The driver applies the
//! calls and runs the frames, and the mailbox holds each client's events and waker.
//!
//! ```
//! use iceblink::CompositorClient;
//! use iceblink_core::CompositorImpl;
//! use iceblink_driver::{DriverOptions, InProcess};
//!
//! let mut driver = InProcess::new(Box::new(CompositorImpl::new()), DriverOptions::default());
//! let (client, _) = driver.register_client();
//! let tx = driver.transaction(client);
//! driver.commit(tx);
//! let mut events = Vec::new();
//! driver.drain_events(client, &mut events);
//! assert_eq!(events.len(), 1);
//! ```
//!
//! # Pacing
//!
//! A paced output's frame is built for a display time. If the presenter has deadlines, the frame
//! latches at the deadline and is built for one interval later. Otherwise it latches one lead
//! before its display time, or just after the latest refresh from which a present still reaches
//! the display at that time when the compositor's own motion wants the frame and that is earlier.
//! The lead is the budget plus the reach, how long before a refresh a present must be made to be
//! shown at it. The display times depend on what the presenter reports:
//!
//! - If the presenter reports when images were shown on the display, or the client reports it
//!   with `report_presented`, the display times are the display's refreshes.
//! - Otherwise a frame's display time is at least one interval after the earliest time the
//!   previous frame could reach the display.
//!
//! Without presenter deadlines, a frame a client's call wants on an idle output latches
//! immediately. An output is idle when no frame is due and the refresh after the newest frame it
//! presented has passed, so an output that shows a frame at every refresh keeps the paced latch.
//! A scroll gesture that a scroll node takes starts the compositor's motion and keeps the paced
//! latch on an idle output too.
//!
//! A frame can reach the display at an earlier refresh than its display time, as a frame that
//! latched immediately often does. Feedback that shows the newest frame early moves the due frame
//! to an earlier refresh where the frames before it now leave room, but never before the display
//! time it was wanted for. [`Driver::next_wake`] includes the time just after the first refresh
//! such a frame can reach, when that feedback can be read.
//!
//! The budget covers the processor's part of a frame. It is the second slowest of the output's
//! recent frame costs plus a quarter, at least the floor and at most one interval. The floor is
//! 1 ms, or half the interval when that is shorter. A frame's cost is the time from its latch to
//! the end of its draw, without the time the draw waited for an image. Before any frame is
//! measured, the budget is what remains of one interval after the reach. An output's first frame
//! after it is made or configured counts at most the floor. `DriverOptions::budget_ns` fixes the
//! budget instead.
//!
//! The reach covers what follows the present: the device's work and the display server's latch.
//! It is measured from the presenter's feedback, as the time from a present to the display
//! between the bounds presents prove. Starting at 3 ms, or the bounds when 3 ms lies outside them,
//! it is lowered step by step while frames reach their refresh, and each step that makes a frame
//! miss is taken back. After the second such miss it stays. A frame that misses once after that,
//! such as one the device was slow to finish, changes nothing, and only repeated misses raise the
//! reach. Without feedback that
//! numbers its presents, the reach stays at 3 ms. A frame that starts too late for its display
//! time by the proven lower bound is built for the next display time it can reach.
//!
//! The interval comes from the first source that has one: the presenter's refresh interval, the
//! one the client set with `set_refresh`, the one the newest feedback reported, the one measured
//! from the display times, and `DriverOptions::interval_ns`.
//!
//! A stepped output builds the frame a client's call wants at the next step, whatever its clock.
//! The compositor's own motion is due one interval of the step clocks after the previous frame,
//! or one interval before the display time a scheduled change needs, whichever is later.

mod core;
mod local;
mod mailbox;
mod options;
mod output;

pub mod thread;

pub use crate::core::Driver;
pub use crate::local::InProcess;
pub use crate::mailbox::{Letter, Mailbox};
pub use crate::options::{DEFAULT_INTERVAL_NS, DriverOptions, Pacing};
pub use iceblink::now_ns;
