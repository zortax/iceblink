//! Host-to-device copies through recycled staging buffers, and lookup tables in textures.
//!
//! An [`UploadBelt`] suballocates writes from mapped staging chunks and records the copies into
//! an encoder. A [`TableTexture`] holds a table of plain-old-data records in an `rgba32uint`
//! texture that shaders read with `textureLoad`.

mod belt;
mod table;

pub use self::belt::{Refused, TexelWrite, UploadBelt};
pub use self::table::{ROW, TEXEL, TEXELS_WIDE, TableTexture, allocate_rows, row_limit, write_run};
