//! Storage that the iceblink implementations build on: arenas with stable addresses and texture
//! atlases.
//!
//! | Module | Contents |
//! | --- | --- |
//! | [`arena`] | Arenas whose values keep their address, and the generation-checked keys that identify them |
//! | [`atlas`] | Texture atlas policy: tile allocation, holds, eviction and deferred uploads |

pub mod arena;
pub mod atlas;
