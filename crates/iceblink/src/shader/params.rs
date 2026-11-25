//! The `Params` a snippet declares, or the empty one it is given.

/// The `Params` of a snippet that declares none.
pub(super) const EMPTY_PARAMS: &str = "struct Params {\n    unused: vec4<f32>,\n}\n";

/// Reports whether `snippet` declares `struct Params`.
pub(super) fn declares_params(snippet: &str) -> bool {
    snippet.match_indices("struct Params").any(|(at, matched)| {
        // `struct ParamsExtra` is another type, and `_struct Params` is no declaration.
        let before = snippet[..at].chars().next_back();
        let after = snippet[at + matched.len()..].chars().next();
        before.is_none_or(|char| !char.is_alphanumeric() && char != '_')
            && after.is_none_or(|char| char.is_whitespace() || char == '{')
    })
}
