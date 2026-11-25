//! Whether a run rewrites goldens.

/// The environment variable that turns a golden comparison into a rewrite.
pub const BLESS: &str = "ICEBLINK_BLESS";

/// Reports whether this run rewrites goldens.
///
/// A run rewrites goldens when `ICEBLINK_BLESS` is set to anything but an empty value or `0`.
pub fn is_blessing() -> bool {
    std::env::var_os(BLESS).is_some_and(|value| !value.is_empty() && value != "0")
}
