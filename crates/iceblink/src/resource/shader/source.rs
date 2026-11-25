//! The programs of application shaders.

use std::sync::Arc;

/// The program of an application shader.
///
/// A renderer uses `prebuilt` when it recognises the format and compiles `wgsl` otherwise, so both
/// describe the same program.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderSource {
    /// The program as WGSL source.
    pub wgsl: Arc<str>,
    /// The program compiled ahead of time.
    pub prebuilt: Option<Prebuilt>,
}

/// A shader program compiled ahead of time.
///
/// The program is the whole module a renderer assembles from the WGSL source.
#[derive(Clone, Debug, PartialEq)]
pub struct Prebuilt {
    /// The encoding of `bytes` and the assembly they were compiled from, such as
    /// `"iceblink-wgsl-0.1.0/naga-30"`.
    pub format: Arc<str>,
    /// The compiled program.
    pub bytes: Arc<[u8]>,
}
