//! A second, naive compositor that implements the shared compositor contract.
//!
//! [`NaiveCompositor`] implements `iceblink::Compositor` in the simplest way the spec's
//! soundness profile allows:
//!
//! - Every frame that changed anything damages the whole output.
//! - Every item is lowered again on every frame, into transient lane instances.
//! - Every instance is a run of one, and every item gets batches of its own, in paint order.
//! - Every frame path gets a vector pass of its own.
//! - Frames carry no shifts, no opaque covers, no pass keys and no resident chunks.
//! - Every frame replaces the four frame tables whole.
//!
//! Glyph runs are rasterised through the `GlyphRaster` the compositor is given, into atlas pages
//! the compositor owns. Images, mark shapes and path masks are placed into pages too. Pages are
//! sent whole to every renderer that does not hold their latest contents.
//!
//! The compositor does not report `precise`, and it implements none of the compositor-driven
//! tier: it rejects animations with `RejectReason::Unsupported`, routes no scroll gesture and hits
//! nothing.

mod commit;
mod compositor;
mod content;
mod frame;
mod geometry;
mod output;
mod registry;
mod scene;
mod space;

pub use crate::naive::compositor::NaiveCompositor;
pub use crate::naive::registry::MemoryFontRegistry;
