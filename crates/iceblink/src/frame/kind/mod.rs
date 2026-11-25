//! The frame kinds: the instance layouts a frame sends to a renderer.
//!
//! In a frame, every slot inside an instance is a frame slot, assigned by the compositor. `clip`
//! indexes the frame clip table or is `FRAME_NO_CLIP`, `space` indexes the frame space table, a
//! `PaintRef` index indexes the frame paint table, and `shader` and `params` index the frame's
//! shaders and parameter table. Frame slots are stable for as long as the object they stand for
//! is drawn.

mod backdrop;
mod external;
mod filter;
mod group;
mod instance;
mod marks;
mod path;
mod shadow;
mod sprite;

pub use self::backdrop::FrameBackdrop;
pub use self::external::FrameExternal;
pub use self::filter::FrameFilter;
pub use self::group::FrameGroup;
pub use self::instance::Instance;
pub use self::marks::{FrameMarkPayload, FrameMarks, MarkCell};
pub use self::path::FramePath;
pub use self::shadow::FrameShadow;
pub use self::sprite::{AtlasTile, ColorSprite, Sprite, SpriteFlags};
