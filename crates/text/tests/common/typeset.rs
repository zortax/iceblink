//! The registry and typesetter the tests lay text out with.

use std::sync::Arc;

use iceblink::FontId;
use iceblink::FontRegistry as _;
use iceblink_testing::fonts::{LATIN, VARIABLE, read_font};
use iceblink_text::typeset::{FontRegistry, Typesetter};

/// The family of the Latin test face.
pub(crate) const SANS: &str = "Noto Sans";

/// The family of the variable test face.
pub(crate) const VARIABLE_FAMILY: &str = "Rubik";

/// A typesetter over a registry of the Latin and the variable test faces, without system fonts.
pub(crate) struct Fonts {
    pub(crate) typesetter: Typesetter,
    pub(crate) latin: FontId,
    pub(crate) variable: FontId,
}

impl Fonts {
    pub(crate) fn new() -> Self {
        let registry = Arc::new(FontRegistry::without_system_fonts());
        let latin = registry.register(read_font(LATIN));
        let variable = registry.register(read_font(VARIABLE));
        Self {
            typesetter: Typesetter::new(registry),
            latin,
            variable,
        }
    }
}
