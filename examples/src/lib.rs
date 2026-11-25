//! Window demos drawn through the reference compositor and renderer.
//!
//! A demo implements [`Demo`] and records its content into a [`Scene`]. [`run`] opens a window,
//! commits what the demo writes, and pumps an in-process driver whenever its next frame latches.
//!
//! The adapter, the presenter, every configure and the renderer's log events are printed to
//! standard output. With [`FRAME_LOG_VARIABLE`] set to `1`, each frame drawn also prints one line
//! of statistics to standard error.
//!
//! The driver paces frames on the refresh interval of the monitor the window is shown on, and a
//! demo that changes over time is advanced to the display time of every frame.
//!
//! The window presents through the renderer's default swap chain, a native Vulkan chain where the
//! device reports when presents reach the display, and wgpu's swap chain elsewhere.
//! `ICEBLINK_PRESENTER` overrides the choice: `native` presents through a native chain where
//! the device can make one, and any other value through wgpu's swap chain.

mod demo;
mod frame_log;
mod items;
mod log;
mod monitor;
mod scene;
mod shell;
mod text;

pub use crate::demo::Demo;
pub use crate::frame_log::{FRAME_LOG_VARIABLE, logs_frames};
pub use crate::items::{quad, solid};
pub use crate::monitor::refresh_ns;
pub use crate::scene::Scene;
pub use crate::shell::run;
pub use crate::text::{Font, Line};
