//! Application shaders, compiled by content and named by the frame's shader slots.

mod compile;
mod registry;
mod slots;

pub use self::compile::check_shader;
pub(crate) use self::registry::{EffectKey, Effects};
pub(crate) use self::slots::EffectSlots;
