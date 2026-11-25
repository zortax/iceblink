//! The modules' text and naga IR, produced by the build script.

use wgpu::naga;

use crate::shader::Module;

macro_rules! ir {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".ir"))
    };
}

macro_rules! text {
    ($name:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/", $name, ".wgsl"))
    };
}

impl Module {
    /// Returns the module's WGSL text.
    pub(crate) fn source(self) -> &'static str {
        match self {
            Self::Quad => text!("quad"),
            Self::Shadow => text!("shadow"),
            Self::Line => text!("line"),
            Self::MonoSprite => text!("mono_sprite"),
            Self::ColorSprite => text!("color_sprite"),
            Self::SubpixelSprite => text!("subpixel_sprite"),
            Self::Blit => text!("blit"),
            Self::Clear => text!("clear"),
            Self::Blur => text!("blur"),
            Self::Composite => text!("composite"),
            Self::External => text!("external"),
            Self::Vector => text!("vector"),
            Self::MarksDisc => text!("marks_disc"),
            Self::MarksBox => text!("marks_box"),
            Self::MarksPolyline => text!("marks_polyline"),
            Self::MarksGlyph => text!("marks_glyph"),
            Self::MarksComposite => text!("marks_composite"),
        }
    }

    /// Returns the module's naga IR, as the build script encoded it.
    fn encoded(self) -> &'static [u8] {
        match self {
            Self::Quad => ir!("quad"),
            Self::Shadow => ir!("shadow"),
            Self::Line => ir!("line"),
            Self::MonoSprite => ir!("mono_sprite"),
            Self::ColorSprite => ir!("color_sprite"),
            Self::SubpixelSprite => ir!("subpixel_sprite"),
            Self::Blit => ir!("blit"),
            Self::Clear => ir!("clear"),
            Self::Blur => ir!("blur"),
            Self::Composite => ir!("composite"),
            Self::External => ir!("external"),
            Self::Vector => ir!("vector"),
            Self::MarksDisc => ir!("marks_disc"),
            Self::MarksBox => ir!("marks_box"),
            Self::MarksPolyline => ir!("marks_polyline"),
            Self::MarksGlyph => ir!("marks_glyph"),
            Self::MarksComposite => ir!("marks_composite"),
        }
    }

    /// Returns the module's naga IR.
    ///
    /// # Panics
    ///
    /// Panics when the IR the build script wrote does not decode.
    pub(crate) fn representation(self) -> naga::Module {
        bincode::deserialize(self.encoded())
            .unwrap_or_else(|error| panic!("{}: {error}", self.label()))
    }
}
