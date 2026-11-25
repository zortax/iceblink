//! The formats of a presented image, the kept target and scratch targets.
//!
//! Targets hold premultiplied gamma-encoded values, so no attachment is an `*Srgb` format that
//! would move blending into linear light. Where a surface offers only encoded formats, the copy
//! into it writes through an unencoded view of the image or cancels the encode itself.

/// How the copy into a presented image keeps the composed values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SrgbTier {
    /// The image's format is unencoded.
    Native,
    /// The image is written through an unencoded view of its encoded format.
    ViewFormatTwin,
    /// The copy decodes each value, so the attachment's encode restores it.
    UndoInBlit,
}

/// The formats chosen for one presenter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Formats {
    /// The presented image's format.
    pub surface: wgpu::TextureFormat,
    /// The kept target's format: the image's format without an encoding suffix.
    pub scene: wgpu::TextureFormat,
    /// The format of scratch targets.
    pub scratch: wgpu::TextureFormat,
    /// The unencoded view format the copy writes through, if any.
    pub view_format_twin: Option<wgpu::TextureFormat>,
    /// How the copy keeps the composed values.
    pub tier: SrgbTier,
    /// How the platform composites the image's alpha.
    pub alpha_mode: wgpu::CompositeAlphaMode,
}

impl Formats {
    /// Chooses the formats for a surface that offers `formats` and `alpha_modes`.
    ///
    /// The surface format is the first of these the surface offers:
    ///
    /// 1. `Bgra8Unorm` or `Rgba8Unorm`, in that order, or on the web in the browser's order,
    /// 2. any unencoded format,
    /// 3. `Bgra8UnormSrgb` or `Rgba8UnormSrgb`, in that order,
    /// 4. the first format offered.
    ///
    /// If the surface offers no formats, the format is `Bgra8UnormSrgb`. An encoded format is
    /// written through its unencoded view when `mutable_texture_formats` is set, and decoded by the
    /// copy otherwise.
    ///
    /// An opaque output takes the opaque alpha mode, and a translucent one premultiplied alpha.
    /// Where the surface does not offer that mode, the output takes opaque if offered, and
    /// otherwise the first mode offered.
    pub fn choose(
        formats: &[wgpu::TextureFormat],
        alpha_modes: &[wgpu::CompositeAlphaMode],
        opaque: bool,
        mutable_texture_formats: bool,
    ) -> Self {
        let (surface, tier) = choose_surface_format(formats, mutable_texture_formats);
        let twin = match tier {
            SrgbTier::ViewFormatTwin => Some(surface.remove_srgb_suffix()),
            SrgbTier::Native | SrgbTier::UndoInBlit => None,
        };
        Self {
            surface,
            scene: surface.remove_srgb_suffix(),
            scratch: wgpu::TextureFormat::Rgba8Unorm,
            view_format_twin: twin,
            tier,
            alpha_mode: choose_alpha_mode(alpha_modes, opaque),
        }
    }

    /// Returns the format the copy writes into the image through.
    pub fn present_attachment(&self) -> wgpu::TextureFormat {
        self.view_format_twin.unwrap_or(self.surface)
    }

    /// Reports whether the copy decodes values before the attachment encodes them.
    pub fn blit_undoes_srgb(&self) -> bool {
        self.tier == SrgbTier::UndoInBlit
    }

    /// Reports whether the image ends up holding the composed values unchanged.
    pub fn is_sound(&self) -> bool {
        !self.surface.is_srgb() || self.blit_undoes_srgb() || self.view_format_twin.is_some()
    }

    /// Returns the view formats the image must be created with.
    pub fn view_formats(&self) -> Vec<wgpu::TextureFormat> {
        self.view_format_twin.into_iter().collect()
    }

    /// Logs the formats chosen on `adapter`.
    pub(crate) fn log(&self, adapter: &str) {
        tracing::info!(
            adapter,
            surface = ?self.surface,
            scene = ?self.scene,
            scratch = ?self.scratch,
            alpha_mode = ?self.alpha_mode,
            tier = ?self.tier,
            "renderer formats"
        );
    }
}

/// Returns the surface format, and how the copy into it keeps the composed values.
fn choose_surface_format(
    formats: &[wgpu::TextureFormat],
    mutable_texture_formats: bool,
) -> (wgpu::TextureFormat, SrgbTier) {
    let preferred = [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ];
    // A browser lists first the format it shows without a copy.
    if cfg!(target_arch = "wasm32")
        && let Some(first) = formats.iter().find(|format| preferred.contains(format))
    {
        return (*first, SrgbTier::Native);
    }
    for candidate in preferred {
        if formats.contains(&candidate) {
            return (candidate, SrgbTier::Native);
        }
    }
    if let Some(plain) = formats.iter().copied().find(|format| !format.is_srgb()) {
        return (plain, SrgbTier::Native);
    }
    // Every offered format encodes, so the encoded twin of a preferred format is taken.
    let encoded = preferred
        .iter()
        .map(|format| format.add_srgb_suffix())
        .find(|format| formats.contains(format))
        .or_else(|| formats.first().copied())
        .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
    let tier = if mutable_texture_formats {
        SrgbTier::ViewFormatTwin
    } else {
        SrgbTier::UndoInBlit
    };
    (encoded, tier)
}

/// Returns the alpha mode an `opaque` or translucent output takes.
fn choose_alpha_mode(
    alpha_modes: &[wgpu::CompositeAlphaMode],
    opaque: bool,
) -> wgpu::CompositeAlphaMode {
    if opaque {
        return pick(alpha_modes, wgpu::CompositeAlphaMode::Opaque);
    }
    if alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
        return wgpu::CompositeAlphaMode::PreMultiplied;
    }
    tracing::info!(
        "the surface offers no premultiplied alpha mode; the window is composited opaque instead"
    );
    pick(alpha_modes, wgpu::CompositeAlphaMode::Opaque)
}

/// Returns `wanted` if `alpha_modes` holds it, and otherwise the first mode offered, or opaque
/// when none is.
fn pick(
    alpha_modes: &[wgpu::CompositeAlphaMode],
    wanted: wgpu::CompositeAlphaMode,
) -> wgpu::CompositeAlphaMode {
    if alpha_modes.contains(&wanted) {
        wanted
    } else {
        alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
    }
}
