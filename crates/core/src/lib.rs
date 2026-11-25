//! The reference compositor.
//!
//! [`CompositorImpl`] implements [`iceblink::Compositor`]. Clients commit transactions of
//! keyed chunks of items under property trees, and the compositor keeps them, derives the damage
//! of every change and builds frames for its outputs.
//!
//! The compositor works in proportion to what changed:
//!
//! - A commit touches the objects its commands name and what depends on them.
//! - A frame visits only the order nodes whose cached ink meets the damage.
//! - A renderer that keeps chunks resident draws a moved chunk again without an upload.
//!
//! The counters of every commit and frame show this work.
//!
//! The compositor is built on three public modules:
//!
//! | Module | Contents |
//! | --- | --- |
//! | [`content`] | The atlas pages and rasterised content that glyph runs, images, marks and small paths lower into |
//! | [`plan`] | Frame planning: draw order, batching, opaque covers, vector passes and mark bins |
//! | [`motion`] | The arithmetic of compositor-driven motion: easings, timing, interpolation and scroll physics |

pub mod content;
pub mod motion;
pub mod plan;

mod client;
mod commit;
mod compositor;
mod derive;
mod font;
mod frame;
mod geometry;
mod hit;
mod invariant;
mod output;
mod resident;
mod scene;
mod shared_content;
mod shift;
mod small;
mod tables;
mod tier;
mod work;

pub use crate::compositor::CompositorImpl;
pub use crate::frame::FrameCheck;
