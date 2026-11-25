//! Assembling an application shader.

use iceblink::ShaderMode;
use iceblink::shader::with_prelude;

use crate::mode::{epilogue, is_rectangle};
use crate::part::part;

/// The format of a prebuilt application shader that iceblink-wgpu decodes.
///
/// The bytes are the naga 30 IR of the module [`effect`] assembles, encoded with bincode 1. The
/// format changes with the version of this crate.
// The workspace pins naga to 30, the version wgpu 30 links.
pub const PREBUILT_FORMAT: &str = concat!("iceblink-wgsl-", env!("CARGO_PKG_VERSION"), "/naga-30");

/// The parts that precede a rectangle shader's text.
const RECTANGLE_PARTS: [&str; 3] = ["common", "sdf", "paint"];

/// Assembles an application shader of `mode` around `snippet`.
///
/// The snippet follows the renderer's shared parts and the prelude, and precedes the epilogue that
/// calls it. A snippet that declares no `struct Params` is given an empty one.
///
/// Returns `None` for a mode the renderer does not know.
pub fn effect(mode: ShaderMode, snippet: &str) -> Option<String> {
    let epilogue = epilogue(mode)?;
    let shared: &[&str] = if is_rectangle(mode) {
        &RECTANGLE_PARTS
    } else {
        &[]
    };

    let head = with_prelude(snippet);
    let mut pieces: Vec<&str> = shared.iter().map(|piece| part(piece)).collect();
    pieces.push(&head);
    pieces.extend(epilogue.iter().map(|piece| part(piece)));

    Some(pieces.join("\n"))
}
