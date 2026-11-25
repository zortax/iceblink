//! The functions a client hands a driver.

use std::sync::Arc;

use crate::{Frame, GpuUnavailable, Renderer};

/// Builds the renderer of an output on the thread that runs the output's frames, or fails when
/// no device can draw it.
pub type RendererFactory = Box<dyn FnOnce() -> Result<Box<dyn Renderer>, GpuUnavailable> + Send>;

/// Work on an output's renderer, run between frames on the thread that draws them.
pub type RendererTask = Box<dyn FnOnce(&mut dyn Renderer) + Send>;

/// Sees every frame built for an output, on the thread that draws it, before the frame is drawn.
pub type FrameObserver = Box<dyn FnMut(&Frame) + Send>;

/// Wakes a client whose events need it.
///
/// A driver may call it on any thread, so it must not wait for the driver.
pub type EventWaker = Arc<dyn Fn() + Send + Sync>;
