//! The renderer's shader modules and the check that their structures match the Rust ones.

mod check;
pub(crate) mod layout;
mod loaders;
mod precompiled;
pub(crate) mod reflect;

pub use self::check::check_shader_layouts;

/// One of the renderer's own shader modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Module {
    Quad,
    Shadow,
    Line,
    MonoSprite,
    ColorSprite,
    SubpixelSprite,
    Blit,
    Clear,
    Blur,
    Composite,
    External,
    Vector,
    MarksDisc,
    MarksBox,
    MarksPolyline,
    MarksGlyph,
    MarksComposite,
}

impl Module {
    pub(crate) const ALL: [Self; 17] = [
        Self::Quad,
        Self::Shadow,
        Self::Line,
        Self::MonoSprite,
        Self::ColorSprite,
        Self::SubpixelSprite,
        Self::Blit,
        Self::Clear,
        Self::Blur,
        Self::Composite,
        Self::External,
        Self::Vector,
        Self::MarksDisc,
        Self::MarksBox,
        Self::MarksPolyline,
        Self::MarksGlyph,
        Self::MarksComposite,
    ];

    /// Reports whether the module reads the frame tables.
    pub(crate) fn uses_tables(self) -> bool {
        !matches!(self, Self::Blit | Self::Clear | Self::Blur)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Quad => "iceblink.shader.quad",
            Self::Shadow => "iceblink.shader.shadow",
            Self::Line => "iceblink.shader.line",
            Self::MonoSprite => "iceblink.shader.mono_sprite",
            Self::ColorSprite => "iceblink.shader.color_sprite",
            Self::SubpixelSprite => "iceblink.shader.subpixel_sprite",
            Self::Blit => "iceblink.shader.blit",
            Self::Clear => "iceblink.shader.clear",
            Self::Blur => "iceblink.shader.blur",
            Self::Composite => "iceblink.shader.composite",
            Self::External => "iceblink.shader.external",
            Self::Vector => "iceblink.shader.vector",
            Self::MarksDisc => "iceblink.shader.marks_disc",
            Self::MarksBox => "iceblink.shader.marks_box",
            Self::MarksPolyline => "iceblink.shader.marks_polyline",
            Self::MarksGlyph => "iceblink.shader.marks_glyph",
            Self::MarksComposite => "iceblink.shader.marks_composite",
        }
    }

    fn is_marks(self) -> bool {
        matches!(
            self,
            Self::MarksDisc
                | Self::MarksBox
                | Self::MarksPolyline
                | Self::MarksGlyph
                | Self::MarksComposite
        )
    }
}
