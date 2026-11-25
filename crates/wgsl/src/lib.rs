//! WGSL modules and application shader assembly.
//!
//! The renderer's pipelines are built from modules, each concatenated from named parts. An
//! application shader is assembled from the parts its mode shares with the renderer, the prelude of
//! [`iceblink::shader`], the application's own text and the epilogue that calls it. Every
//! module-scope name the renderer's parts add to an application shader begins with
//! [`RESERVED_PREFIX`](iceblink::shader::RESERVED_PREFIX).

mod effect;
mod mode;
mod module;
mod part;

pub use crate::effect::{PREBUILT_FORMAT, effect};
pub use crate::mode::{epilogue, fragment_entry, is_rectangle, vertex_entry};
pub use crate::module::{MODULES, PRELUDES, module};
pub use crate::part::{PART_NAMES, part};
