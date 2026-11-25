//! Application shaders.

mod source;

use crate::bits::{bit_set, enumeration};

pub use self::source::{Prebuilt, ShaderSource};

/// An application shader: its mode, its program and the inputs it reads.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderDesc {
    /// The function the program defines, and where it is drawn.
    pub mode: ShaderMode,
    /// The program.
    pub source: ShaderSource,
    /// The size of the shader's own parameters in bytes, at most 64.
    pub params_size: u32,
    /// The inputs the shader reads that change without a client write.
    pub reads: ShaderReads,
}

enumeration! {
    /// The function an application shader defines, and where it is drawn.
    pub struct ShaderMode(u8) {
        /// `shade` returns the colour of a shader quad.
        PAINT = 0,
        /// `coverage` returns the coverage of a shader quad, which its paints fill.
        COVERAGE = 1,
        /// `apply` returns the filtered colour of a group, as a filter of an effect.
        FILTER = 2,
    }
}

bit_set! {
    /// The inputs an application shader reads that change without a client write.
    pub struct ShaderReads(u8) {
        /// The clock, so the shader's users are drawn again on every frame.
        TIME = 1 << 0,
    }
}
