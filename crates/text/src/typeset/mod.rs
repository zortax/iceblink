//! Text layout over Parley, recorded as compositor items.
//!
//! [`FontRegistry`] implements [`iceblink::FontRegistry`] over a fontique collection of system
//! fonts and registered font files. Once it is shared with the compositor through
//! [`Compositor::use_font_registry`](iceblink::Compositor::use_font_registry), layout and the
//! compositor's glyph cache identify a font by the same [`FontId`](iceblink::FontId).
//!
//! A [`Typesetter`] lays out a text with spans of [`Property`] into a [`Layout`]: glyph runs with
//! variation coordinates and synthesis, and decoration lines, broken into lines at a width and
//! aligned. A layout also computes the rectangles of a selection, the rectangle of a caret, and
//! the [`Position`] under a point.
//!
//! A [`TextRecorder`] records layouts, selections and carets into an
//! [`iceblink::ChunkRecorder`].

mod layout;
mod record;
mod registry;
mod style;
mod typesetter;

pub use parley::{Affinity, Alignment, FontStyle};

pub use crate::typeset::layout::{Layout, Position, TextItem, TextRun};
pub use crate::typeset::record::TextRecorder;
pub use crate::typeset::registry::FontRegistry;
pub use crate::typeset::style::{Property, TextPaint};
pub use crate::typeset::typesetter::{TextBuilder, Typesetter};
