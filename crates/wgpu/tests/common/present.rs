//! Renderers over any presenter, holding the device lock and validating every frame.

use std::sync::MutexGuard;

use iceblink::Renderer as _;
use iceblink::{Frame, FrameOutcome, RenderTarget};
use iceblink_testing::FrameValidator;
use iceblink_wgpu::wgpu::Texture;
use iceblink_wgpu::{Offscreen, Pixels, Supplied, WgpuRenderer};

use super::device::{SIDE, validated_draw};

/// A renderer, the validator its frames pass through, and the device lock.
pub(crate) struct Rig {
    pub(crate) renderer: WgpuRenderer,
    validator: FrameValidator,
    _guard: Option<MutexGuard<'static, ()>>,
}

impl std::ops::Deref for Rig {
    type Target = WgpuRenderer;

    fn deref(&self) -> &WgpuRenderer {
        &self.renderer
    }
}

impl std::ops::DerefMut for Rig {
    fn deref_mut(&mut self) -> &mut WgpuRenderer {
        &mut self.renderer
    }
}

impl Rig {
    /// Returns a rig over `renderer`, configured at the side of every test output.
    pub(crate) fn new(renderer: WgpuRenderer, guard: MutexGuard<'static, ()>) -> Self {
        Self::configured(renderer, Some(guard))
    }

    /// Returns a rig over `renderer` while another rig holds the device lock.
    pub(crate) fn beside(renderer: WgpuRenderer) -> Self {
        Self::configured(renderer, None)
    }

    /// Returns a rig over `renderer` that holds `guard`, configured at the side of every test
    /// output.
    fn configured(mut renderer: WgpuRenderer, guard: Option<MutexGuard<'static, ()>>) -> Self {
        eprintln!("adapter: {}", renderer.gpu().describe());
        renderer.configure(target(SIDE));
        Self {
            renderer,
            validator: FrameValidator::new(),
            _guard: guard,
        }
    }

    /// Returns the renderer's offscreen presenter.
    pub(crate) fn offscreen(&mut self) -> &mut Offscreen {
        self.renderer
            .wgpu_presenter()
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<Offscreen>())
            .expect("the renderer presents offscreen")
    }

    /// Checks `frame` against the frame contract and draws it.
    ///
    /// # Panics
    ///
    /// Panics when the frame breaks the contract.
    pub(crate) fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        validated_draw(&mut self.renderer, &mut self.validator, frame)
    }

    /// Draws `frame`, asserts it was presented, and reads the presented image back.
    pub(crate) fn present(&mut self, frame: &Frame) -> Pixels {
        let outcome = self.draw(frame);
        assert!(
            outcome.stats().is_some(),
            "the frame was not presented: {outcome:?}"
        );
        self.renderer
            .read_presented()
            .expect("the presented image can be read back")
    }

    /// Returns the renderer's supplied presenter.
    pub(crate) fn supplied(&mut self) -> &mut Supplied {
        self.renderer
            .wgpu_presenter()
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<Supplied>())
            .expect("the renderer presents into supplied textures")
    }

    /// Points the next frame at the supplied texture at `slot`.
    pub(crate) fn select(&mut self, slot: u32) {
        assert!(self.supplied().select(slot), "slot {slot} exists");
    }

    /// Reads `texture` back.
    pub(crate) fn read(&self, texture: &Texture) -> Pixels {
        Pixels::of_texture(self.renderer.gpu(), texture)
    }

    /// Asserts that `texture` holds exactly what the kept target holds.
    pub(crate) fn assert_holds_composed(&self, texture: &Texture, what: &str) {
        let difference = self
            .read(texture)
            .max_difference(&self.renderer.read_composed());
        assert_eq!(
            difference, 0,
            "{what}: the image differs from the kept target"
        );
    }
}

/// Returns an opaque square target of `side` pixels at scale 1.
pub(crate) fn target(side: u32) -> RenderTarget {
    RenderTarget {
        size: [side, side],
        scale: 1.0,
        opaque: true,
    }
}
