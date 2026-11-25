//! The renderer's pipelines and what each one draws, binds and blends.

use crate::pipeline::marks::MarkKind;
use crate::shader::Module;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PipelineKind {
    Quad,
    Shadow,
    Line,
    MonoSprite,
    ColorSprite,
    SubpixelSprite,
    Blit,
    BlitUndoSrgb,
    DamageClear,
    BlurDownsample,
    BlurAxis,
    Composite,
    External,
    VectorComposite,
    MarksDisc,
    MarksDiscCoverage,
    MarksBox,
    MarksBoxCoverage,
    MarksPolyline,
    MarksPolylineCoverage,
    MarksGlyph,
    MarksGlyphCoverage,
    MarksComposite,
}

impl PipelineKind {
    pub(crate) const ALL: [Self; 23] = [
        Self::Quad,
        Self::Shadow,
        Self::Line,
        Self::MonoSprite,
        Self::ColorSprite,
        Self::SubpixelSprite,
        Self::Blit,
        Self::BlitUndoSrgb,
        Self::DamageClear,
        Self::BlurDownsample,
        Self::BlurAxis,
        Self::Composite,
        Self::External,
        Self::VectorComposite,
        Self::MarksDisc,
        Self::MarksDiscCoverage,
        Self::MarksBox,
        Self::MarksBoxCoverage,
        Self::MarksPolyline,
        Self::MarksPolylineCoverage,
        Self::MarksGlyph,
        Self::MarksGlyphCoverage,
        Self::MarksComposite,
    ];

    /// The format of mark bin pages, which the coverage kinds draw into.
    pub(crate) const COVERAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Returns the kind that draws marks of `kind`, into a bin page when `coverage` is set.
    pub(crate) fn marks(kind: MarkKind, coverage: bool) -> Self {
        match (kind, coverage) {
            (MarkKind::Disc, false) => Self::MarksDisc,
            (MarkKind::Disc, true) => Self::MarksDiscCoverage,
            (MarkKind::Box, false) => Self::MarksBox,
            (MarkKind::Box, true) => Self::MarksBoxCoverage,
            (MarkKind::Polyline, false) => Self::MarksPolyline,
            (MarkKind::Polyline, true) => Self::MarksPolylineCoverage,
            (MarkKind::Glyph, false) => Self::MarksGlyph,
            (MarkKind::Glyph, true) => Self::MarksGlyphCoverage,
        }
    }

    pub(crate) fn module(self) -> Module {
        match self {
            Self::Quad => Module::Quad,
            Self::Shadow => Module::Shadow,
            Self::Line => Module::Line,
            Self::MonoSprite => Module::MonoSprite,
            Self::ColorSprite => Module::ColorSprite,
            Self::SubpixelSprite => Module::SubpixelSprite,
            Self::Blit | Self::BlitUndoSrgb => Module::Blit,
            Self::DamageClear => Module::Clear,
            Self::BlurDownsample | Self::BlurAxis => Module::Blur,
            Self::Composite => Module::Composite,
            Self::External => Module::External,
            Self::VectorComposite => Module::Vector,
            Self::MarksDisc | Self::MarksDiscCoverage => Module::MarksDisc,
            Self::MarksBox | Self::MarksBoxCoverage => Module::MarksBox,
            Self::MarksPolyline | Self::MarksPolylineCoverage => Module::MarksPolyline,
            Self::MarksGlyph | Self::MarksGlyphCoverage => Module::MarksGlyph,
            Self::MarksComposite => Module::MarksComposite,
        }
    }

    /// Reports whether the kind reads a draw order as its vertex buffer.
    pub(crate) fn takes_order(self) -> bool {
        matches!(
            self,
            Self::Quad
                | Self::Shadow
                | Self::Line
                | Self::MonoSprite
                | Self::ColorSprite
                | Self::SubpixelSprite
        )
    }

    pub(crate) fn vertex_entry(self) -> &'static str {
        match self {
            Self::Quad => "vs_quad",
            Self::Shadow => "vs_shadow",
            Self::Line => "vs_line",
            Self::MonoSprite => "vs_mono_sprite",
            Self::ColorSprite => "vs_color_sprite",
            Self::SubpixelSprite => "vs_subpixel_sprite",
            Self::Blit | Self::BlitUndoSrgb => "vs_blit",
            Self::DamageClear => "vs_clear",
            Self::BlurDownsample | Self::BlurAxis => "vs_blur",
            Self::Composite => "vs_composite",
            Self::External => "vs_external",
            Self::VectorComposite => "vs_vector",
            Self::MarksDisc => "vs_disc_paint",
            Self::MarksDiscCoverage => "vs_disc_coverage",
            Self::MarksBox => "vs_box_paint",
            Self::MarksBoxCoverage => "vs_box_coverage",
            Self::MarksPolyline => "vs_segment_paint",
            Self::MarksPolylineCoverage => "vs_segment_coverage",
            Self::MarksGlyph => "vs_glyph_paint",
            Self::MarksGlyphCoverage => "vs_glyph_coverage",
            Self::MarksComposite => "vs_mark_composite",
        }
    }

    pub(crate) fn fragment_entry(self) -> &'static str {
        match self {
            Self::Quad => "fs_quad",
            Self::Shadow => "fs_shadow",
            Self::Line => "fs_line",
            Self::MonoSprite => "fs_mono_sprite",
            Self::ColorSprite => "fs_color_sprite",
            Self::SubpixelSprite => "fs_subpixel_sprite",
            Self::Blit => "fs_blit",
            Self::BlitUndoSrgb => "fs_blit_undo_srgb",
            Self::DamageClear => "fs_clear",
            Self::BlurDownsample => "fs_blur_downsample",
            Self::BlurAxis => "fs_blur_axis",
            Self::Composite => "fs_composite",
            Self::External => "fs_external",
            Self::VectorComposite => "fs_vector",
            Self::MarksDisc => "fs_disc_paint",
            Self::MarksDiscCoverage => "fs_disc_coverage",
            Self::MarksBox => "fs_box_paint",
            Self::MarksBoxCoverage => "fs_box_coverage",
            Self::MarksPolyline => "fs_segment_paint",
            Self::MarksPolylineCoverage => "fs_segment_coverage",
            Self::MarksGlyph => "fs_glyph_paint",
            Self::MarksGlyphCoverage => "fs_glyph_coverage",
            Self::MarksComposite => "fs_mark_composite",
        }
    }

    pub(crate) fn samples_atlas(self) -> bool {
        matches!(
            self,
            Self::MonoSprite
                | Self::ColorSprite
                | Self::SubpixelSprite
                | Self::MarksGlyph
                | Self::MarksGlyphCoverage
        )
    }

    pub(crate) fn is_instanced(self) -> bool {
        matches!(
            self,
            Self::Quad
                | Self::Shadow
                | Self::Line
                | Self::MonoSprite
                | Self::ColorSprite
                | Self::SubpixelSprite
        )
    }

    pub(crate) fn uses_tables(self) -> bool {
        self.module().uses_tables()
    }

    /// Reports whether the kind samples a texture through a filtering sampler, with a uniform
    /// block.
    pub(crate) fn samples_through_block(self) -> bool {
        matches!(
            self,
            Self::BlurDownsample | Self::BlurAxis | Self::Composite | Self::External
        )
    }

    pub(crate) fn composites_vector(self) -> bool {
        self == Self::VectorComposite
    }

    pub(crate) fn draws_marks(self) -> bool {
        matches!(
            self,
            Self::MarksDisc
                | Self::MarksDiscCoverage
                | Self::MarksBox
                | Self::MarksBoxCoverage
                | Self::MarksPolyline
                | Self::MarksPolylineCoverage
                | Self::MarksGlyph
                | Self::MarksGlyphCoverage
                | Self::MarksComposite
        )
    }

    /// Reports whether the kind adds coverage into a mark bin page.
    pub(crate) fn adds_coverage(self) -> bool {
        matches!(
            self,
            Self::MarksDiscCoverage
                | Self::MarksBoxCoverage
                | Self::MarksPolylineCoverage
                | Self::MarksGlyphCoverage
        )
    }

    /// Reports whether the kind can draw into targets of `format`.
    pub(crate) fn suits(self, format: wgpu::TextureFormat) -> bool {
        if self.adds_coverage() && format != Self::COVERAGE_FORMAT {
            return false;
        }
        self != Self::SubpixelSprite || format != crate::target::group_pool::GroupPool::FORMAT
    }

    pub(crate) fn needs_dual_source_blending(self) -> bool {
        self == Self::SubpixelSprite
    }

    pub(crate) fn blend(self) -> Option<wgpu::BlendState> {
        match self {
            // A copy, a clear and a filtering pass replace what the attachment holds, so their
            // result never depends on it.
            Self::Blit
            | Self::BlitUndoSrgb
            | Self::DamageClear
            | Self::BlurDownsample
            | Self::BlurAxis => None,
            // Fill coverage adds up and the format saturates the sum at one, which gives the
            // union of the fills, exact where two fills abut.
            Self::MarksDiscCoverage | Self::MarksBoxCoverage | Self::MarksGlyphCoverage => {
                Some(Self::coverage_blend(wgpu::BlendOperation::Add))
            }
            Self::MarksPolylineCoverage => Some(Self::coverage_blend(wgpu::BlendOperation::Max)),
            Self::SubpixelSprite => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Src1,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            _ => Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        }
    }

    fn coverage_blend(operation: wgpu::BlendOperation) -> wgpu::BlendState {
        let component = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation,
        };
        wgpu::BlendState {
            color: component,
            alpha: component,
        }
    }

    pub(crate) fn write_mask(self) -> wgpu::ColorWrites {
        if self == Self::SubpixelSprite {
            wgpu::ColorWrites::COLOR
        } else {
            wgpu::ColorWrites::ALL
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Quad => "iceblink.pipeline.quad",
            Self::Shadow => "iceblink.pipeline.shadow",
            Self::Line => "iceblink.pipeline.line",
            Self::MonoSprite => "iceblink.pipeline.mono_sprite",
            Self::ColorSprite => "iceblink.pipeline.color_sprite",
            Self::SubpixelSprite => "iceblink.pipeline.subpixel_sprite",
            Self::Blit => "iceblink.pipeline.blit",
            Self::BlitUndoSrgb => "iceblink.pipeline.blit_undo_srgb",
            Self::DamageClear => "iceblink.pipeline.damage_clear",
            Self::BlurDownsample => "iceblink.pipeline.blur_downsample",
            Self::BlurAxis => "iceblink.pipeline.blur_axis",
            Self::Composite => "iceblink.pipeline.composite",
            Self::External => "iceblink.pipeline.external",
            Self::VectorComposite => "iceblink.pipeline.vector_composite",
            Self::MarksDisc => "iceblink.pipeline.marks_disc",
            Self::MarksDiscCoverage => "iceblink.pipeline.marks_disc_coverage",
            Self::MarksBox => "iceblink.pipeline.marks_box",
            Self::MarksBoxCoverage => "iceblink.pipeline.marks_box_coverage",
            Self::MarksPolyline => "iceblink.pipeline.marks_polyline",
            Self::MarksPolylineCoverage => "iceblink.pipeline.marks_polyline_coverage",
            Self::MarksGlyph => "iceblink.pipeline.marks_glyph",
            Self::MarksGlyphCoverage => "iceblink.pipeline.marks_glyph_coverage",
            Self::MarksComposite => "iceblink.pipeline.marks_composite",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineKind;

    #[test]
    fn only_the_per_channel_pipeline_needs_a_feature_or_withholds_alpha() {
        let gated: Vec<PipelineKind> = PipelineKind::ALL
            .into_iter()
            .filter(|kind| kind.needs_dual_source_blending())
            .collect();
        assert_eq!(gated, vec![PipelineKind::SubpixelSprite]);
        assert_eq!(
            PipelineKind::SubpixelSprite.write_mask(),
            wgpu::ColorWrites::COLOR
        );
        for kind in PipelineKind::ALL {
            if kind == PipelineKind::SubpixelSprite {
                continue;
            }
            assert_eq!(kind.write_mask(), wgpu::ColorWrites::ALL, "{kind:?}");
        }
    }

    #[test]
    fn everything_that_composites_blends_and_everything_that_replaces_does_not() {
        let replacing = [
            PipelineKind::Blit,
            PipelineKind::BlitUndoSrgb,
            PipelineKind::DamageClear,
            PipelineKind::BlurDownsample,
            PipelineKind::BlurAxis,
        ];
        for kind in PipelineKind::ALL {
            assert_eq!(
                kind.blend().is_none(),
                replacing.contains(&kind),
                "{kind:?} blends inconsistently with what it is"
            );
        }
    }

    #[test]
    fn the_per_channel_pipeline_is_the_one_kind_the_pool_format_may_not_have() {
        use crate::target::group_pool::GroupPool;

        let refused: Vec<PipelineKind> = PipelineKind::ALL
            .into_iter()
            .filter(|kind| !kind.suits(GroupPool::FORMAT) && !kind.adds_coverage())
            .collect();
        assert_eq!(refused, vec![PipelineKind::SubpixelSprite]);
        for kind in PipelineKind::ALL {
            assert_eq!(
                kind.suits(wgpu::TextureFormat::Bgra8Unorm),
                !kind.adds_coverage(),
                "{kind:?} is refused for the composed target"
            );
        }
    }

    #[test]
    fn a_coverage_draw_writes_only_the_bin_pages() {
        for kind in PipelineKind::ALL {
            if !kind.adds_coverage() {
                continue;
            }
            assert!(kind.suits(PipelineKind::COVERAGE_FORMAT), "{kind:?}");
            assert!(kind.draws_marks(), "{kind:?}");
            let blend = kind.blend().expect("coverage blends");
            assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::One, "{kind:?}");
            // The segments of a stroke keep their largest coverage, and fills add theirs.
            let operation = if kind == PipelineKind::MarksPolylineCoverage {
                wgpu::BlendOperation::Max
            } else {
                wgpu::BlendOperation::Add
            };
            assert_eq!(blend.color.operation, operation, "{kind:?}");
        }
    }

    #[test]
    fn a_pipeline_binds_the_tables_exactly_when_its_module_declares_them() {
        for kind in PipelineKind::ALL {
            assert_eq!(
                kind.uses_tables(),
                kind.is_instanced()
                    || kind.draws_marks()
                    || kind.composites_vector()
                    || kind.samples_through_block()
                        && kind != PipelineKind::BlurDownsample
                        && kind != PipelineKind::BlurAxis,
                "{kind:?}"
            );
        }
    }
}
