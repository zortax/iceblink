//! Resources: images, external textures and application shaders.
//!
//! Resources are created and written through transactions, and removed with
//! `Transaction::remove`.

mod image;
mod shader;
mod texture;

pub use self::image::{ImageDesc, ImageFormat, ImageWrite};
pub use self::shader::{Prebuilt, ShaderDesc, ShaderMode, ShaderReads, ShaderSource};
pub use self::texture::TextureDesc;
