//! The WGSL contract of application shaders.
//!
//! An application shader is a WGSL snippet that defines the function its mode calls. A renderer
//! compiles the snippet together with the prelude and its own code. Every module-scope name the
//! renderer adds begins with [`RESERVED_PREFIX`], and a snippet declares no such name and none of
//! the prelude's.
//!
//! A client checks a snippet without a renderer by validating its [`validation_module`] with a
//! WGSL front end such as naga.

mod checks;
mod params;

use crate::ShaderMode;

/// The WGSL that declares the input of an application shader and the helpers it may use.
///
/// The prelude reads no binding, so it is the same in every mode.
pub const PRELUDE: &str = include_str!("prelude.wgsl");

/// The most bytes of parameters an application shader declares.
pub const MAX_PARAMS_BYTES: usize = 64;

/// The prefix of every module-scope name a renderer adds to an application shader.
pub const RESERVED_PREFIX: &str = "cr_";

/// Returns the name of the function an application shader of `mode` defines.
///
/// Returns `None` for a mode the contract does not define.
pub fn entry(mode: ShaderMode) -> Option<&'static str> {
    match mode {
        ShaderMode::PAINT => Some("shade"),
        ShaderMode::COVERAGE => Some("coverage"),
        ShaderMode::FILTER => Some("apply"),
        _ => None,
    }
}

/// Returns `snippet` preceded by the prelude, and by an empty `Params` if the snippet declares
/// none.
pub fn with_prelude(snippet: &str) -> String {
    let mut source = String::with_capacity(PRELUDE.len() + snippet.len() + 64);
    source.push_str(PRELUDE);
    source.push('\n');
    if !params::declares_params(snippet) {
        source.push_str(params::EMPTY_PARAMS);
        source.push('\n');
    }
    source.push_str(snippet);
    source
}

/// Returns the WGSL module a client validates an application shader of `mode` with.
///
/// The module is [`with_prelude`] of the snippet, followed by declarations that check what a
/// renderer relies on:
///
/// - `Params` fits the parameter block of [`MAX_PARAMS_BYTES`] under WGSL's uniform layout rules.
/// - The function named by [`entry`] takes the arguments and returns the type its mode calls for.
///
/// The module has no entry point, so it is validated and never drawn.
///
/// Returns `None` for a mode the contract does not define.
pub fn validation_module(mode: ShaderMode, snippet: &str) -> Option<String> {
    let signature = checks::signature(mode)?;
    let mut source = with_prelude(snippet);
    source.push('\n');
    source.push_str(&checks::params_block());
    source.push('\n');
    source.push_str(signature);
    Some(source)
}
