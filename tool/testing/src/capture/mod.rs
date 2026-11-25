//! A recording renderer and stable frame transcripts.
//!
//! [`CaptureRenderer`] implements `iceblink::Renderer` without a device. It applies each
//! frame's page, shader, note and table work to a [`Mirror`] of what a renderer holds, and keeps
//! the frame with two [`Transcript`]s: the whole frame, and what it draws. A transcript resolves
//! every instance through the mirror, so it holds no raw slots.

pub mod mirror;
pub mod renderer;
pub mod text;
pub mod transcript;

pub use crate::capture::mirror::Mirror;
pub use crate::capture::renderer::{CaptureRenderer, Captured, copy_frame};
pub use crate::capture::transcript::Transcript;
