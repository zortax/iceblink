//! Runs a compositor, the renderers of its outputs and their presenters on a driver thread.
//!
//! [`ThreadedCompositor::spawn`] moves a `Compositor` onto a new driver thread and returns a
//! [`ClientHandle`], the [`CompositorClient`](iceblink::CompositorClient) of the thread.
//! Clients clone the handle and use it from any thread. Calls through every clone are queued on
//! one channel and applied by the driver between frames, in the order they were made.
//!
//! The driver builds, draws and presents the frames of every output, paced as
//! [Pacing](crate#pacing) describes. Each output's renderer is built on the driver thread by a
//! `RendererFactory` and never leaves it, so a renderer need not be `Send`. Animations and scroll
//! motion of a compositor with the compositor-driven tier keep running at the display's rate
//! while the client's threads are busy.
//!
//! ```
//! use iceblink::{ClientEvent, CompositorClient};
//! use iceblink_core::CompositorImpl;
//! use iceblink_driver::thread::ThreadedCompositor;
//!
//! let (driver, mut handle) = ThreadedCompositor::spawn(Box::new(CompositorImpl::new()));
//! assert!(handle.capabilities().threaded);
//! let (client, _) = handle.register_client();
//! let tx = handle.transaction(client);
//! handle.commit(tx);
//! handle.sync();
//! let mut events = Vec::new();
//! handle.drain_events(client, &mut events);
//! assert!(matches!(events[..], [ClientEvent::Committed { .. }]));
//! driver.shutdown();
//! ```
//!
//! # Publication
//!
//! After every batch of calls and frames, the driver delivers the events they caused, returns the
//! buffers of applied commits as the clients' next transactions, and publishes a snapshot of
//! itself, the compositor and every output. It then calls the wakers the events owe, on the
//! driver thread.
//!
//! A batch ends when no call is queued, when a frame latches, after a step, or after 64 calls. A
//! frame that latched immediately on an idle output ends the batch only when no call is queued or
//! after 64 calls, so it shows the calls queued behind the one that latched it.
//!
//! # Registration
//!
//! The driver keeps two clients registered in reserve. `register_client` takes one without
//! waiting, and the driver registers another between its frames. A registration that finds no
//! client in reserve waits for the driver.

mod command;
mod driver;
mod handle;
mod shared;
mod threaded;

pub use crate::thread::handle::ClientHandle;
pub use crate::thread::threaded::ThreadedCompositor;
