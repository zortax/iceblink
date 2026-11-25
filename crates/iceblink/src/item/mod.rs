//! The client items: drawable things recorded into chunks.
//!
//! Every kind except [`PathItem`] is a `#[repr(C)]` Pod struct without padding, so a recorded item
//! is copied as bytes. Items hold no paint order and no kind header. They store the ids of nodes
//! and resources as raw `u32` slots.

mod arena;
mod backdrop;
mod external;
mod glyph_run;
mod image;
mod kind;
mod line;
mod marks;
mod path;
mod quad;
mod refs;
mod shader_quad;
mod shadow;
mod shape;

pub use self::arena::ArenaRange;
pub use self::backdrop::Backdrop;
pub use self::external::ExternalQuad;
pub use self::glyph_run::{Glyph, GlyphFlags, GlyphRun};
pub use self::image::{Extend, Image, ImageFilter, ImageFlags};
pub use self::kind::{ItemKind, ItemOp};
pub use self::line::{Line, LineStyle};
pub use self::marks::{MarkBox, MarkFlags, MarkPayload, MarkPlacement, Marks};
pub use self::path::{FillRule, PathClip, PathFill, PathItem, PathStroke};
pub use self::quad::{BorderStyle, Quad};
pub use self::refs::{ClipRef, SpaceRef};
pub use self::shader_quad::ShaderQuad;
pub use self::shadow::Shadow;
pub use self::shape::{CornerShapes, ROUND};
