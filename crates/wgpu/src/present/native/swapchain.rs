//! A native chain with the debt of each image and the image a frame is copied into.

use std::sync::Arc;

use iceblink::STALE_DAMAGE;
use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Point, Rect, Size};

use crate::gpu::device::Gpu;
use crate::present::acquisition::Acquisition;
use crate::present::debt::ImageDebts;
use crate::present::native::backend::Backend;
use crate::present::native::{ChainConfig, NativeWindow};

/// A native chain whose images are identified by their slot in the chain.
///
/// Every image owes the whole target after a configuration and after the chain reports it out of
/// date or lost. A present reports the whole frame as changed after a configuration, after a
/// present that failed, and when the frame's extent changed since the previous present.
#[derive(Debug)]
pub(crate) struct NativeSwapchain {
    gpu: Arc<Gpu>,
    /// The view of each image, by slot, once an acquisition made it.
    // The views and the held image are dropped before the swap chain whose images they view.
    views: Vec<Option<wgpu::TextureView>>,
    held: Option<Held>,
    backend: Backend,
    debts: ImageDebts,
    /// The extent of the frames copied into the images, at most the images' own.
    frame: [u32; 2],
    /// The frame extent of the last present, or `None` when the next one covers the whole frame.
    presented: Option<[u32; 2]>,
    /// Whether presented images are checked against the kept target.
    verify: bool,
}

/// The acquired image, which a release parks until the next acquisition.
#[derive(Debug)]
struct Held {
    slot: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Whether the image is parked.
    released: bool,
}

impl NativeSwapchain {
    /// Returns a chain for `surface` on `gpu`, or `None` where the backend or the surface cannot
    /// have one.
    pub(crate) fn new(
        gpu: &Arc<Gpu>,
        surface: &wgpu::Surface<'static>,
        window: NativeWindow,
    ) -> Option<Self> {
        let backend = Backend::new(gpu, surface, window)?;
        Some(Self {
            gpu: Arc::clone(gpu),
            views: Vec::new(),
            held: None,
            backend,
            debts: ImageDebts::default(),
            frame: [1, 1],
            presented: None,
            verify: crate::present::native::verifies(),
        })
    }

    /// Makes the swap chain again for `config`, for frames of `frame` texels.
    ///
    /// Fails with the reason the chain cannot be made.
    pub(crate) fn configure(
        &mut self,
        config: &ChainConfig,
        frame: [u32; 2],
    ) -> Result<(), String> {
        self.held = None;
        self.views.clear();
        let images = self.backend.configure(config)?;
        self.debts = ImageDebts::new(images);
        self.presented = None;
        self.frame = frame;
        Ok(())
    }

    /// Sets the extent of the frames copied into the images.
    pub(crate) fn set_frame(&mut self, frame: [u32; 2]) {
        self.frame = frame;
    }

    /// Acquires the image the next frame is copied into, viewed in `view_format`.
    ///
    /// A parked image is returned again. The result holds the image's slot and whether the
    /// chain asks to be configured again.
    ///
    /// Fails with the answer when no image was acquired.
    pub(crate) fn acquire(
        &mut self,
        view_format: wgpu::TextureFormat,
    ) -> Result<(u32, bool), Acquisition> {
        if let Some(held) = &mut self.held {
            held.released = false;
            return Ok((held.slot, false));
        }
        let (answer, image) = self.backend.acquire();
        let Some((slot, texture)) = image.filter(|_| answer.presents()) else {
            self.forget_after(answer);
            return Err(answer);
        };
        let view = self.view_of(slot, &texture, view_format);
        self.held = Some(Held {
            slot,
            texture,
            view,
            released: false,
        });
        Ok((slot, answer == Acquisition::Suboptimal))
    }

    /// Returns the slot of the image acquired and neither presented nor released, if any.
    pub(crate) fn acquired(&self) -> Option<u32> {
        self.held
            .as_ref()
            .filter(|held| !held.released)
            .map(|held| held.slot)
    }

    /// Parks the acquired image at `slot`.
    // A swap chain image cannot be handed back without presenting it.
    pub(crate) fn release(&mut self, slot: u32) {
        if let Some(held) = self.held.as_mut().filter(|held| held.slot == slot) {
            held.released = true;
        }
    }

    /// Returns the texture of the acquired image at `slot`.
    pub(crate) fn texture(&self, slot: u32) -> Option<&wgpu::Texture> {
        self.current(slot).map(|held| &held.texture)
    }

    /// Returns the view of the acquired image at `slot`.
    pub(crate) fn view(&self, slot: u32) -> Option<&wgpu::TextureView> {
        self.current(slot).map(|held| &held.view)
    }

    /// Adds a composed frame's present damage to every image's debt.
    pub(crate) fn composed(&mut self, damage: &DamageSet) {
        self.debts.composed(damage);
    }

    /// Returns what a copy into the image at `slot` covers.
    pub(crate) fn debt(&self, slot: u32) -> &DamageSet<STALE_DAMAGE> {
        self.debts.debt(slot)
    }

    /// Presents the acquired image at `slot`, with `damage` as what changed since the previous
    /// present, and clears its debt when the present succeeds.
    ///
    /// Returns `None` when the image at `slot` is not acquired.
    pub(crate) fn present(&mut self, slot: u32, damage: &DamageSet) -> Option<Acquisition> {
        self.current(slot)?;
        self.held = None;
        let extent = self.backend.extent();
        let frame = [self.frame[0].min(extent[0]), self.frame[1].min(extent[1])];
        // A changed frame extent changes what the display server scales the image from, so such a
        // present covers the whole frame.
        let rects = if self.presented == Some(frame) {
            changed_rects(damage, frame)
        } else {
            None
        };
        let answer = self.backend.present(slot, rects.as_deref());
        if answer.presents() {
            self.debts.paid(slot);
            self.presented = Some(frame);
        } else {
            // The display server never showed this frame, so the next present is compared with
            // one it did show.
            self.presented = None;
            self.forget_after(answer);
        }
        Some(answer)
    }

    /// Reports whether a present passes the changed rectangles to the display server.
    pub(crate) fn partial_present(&self) -> bool {
        self.backend.partial()
    }

    /// Reports whether the chain reports when its presents were shown on the display.
    pub(crate) fn timed(&self) -> bool {
        self.backend.timed()
    }

    /// Returns the refresh interval of the display the chain presents to, where known.
    pub(crate) fn refresh_ns(&self) -> Option<u64> {
        self.backend.refresh_ns()
    }

    /// Returns the number of the newest present, or zero when the chain numbers none.
    pub(crate) fn presents(&self) -> u64 {
        self.backend.presents()
    }

    /// Takes the refreshes frames are planned on: one at `refresh_ns` and the others every
    /// `interval_ns` from it, on the frame clock.
    pub(crate) fn follow_refreshes(&self, refresh_ns: u64, interval_ns: u64) {
        self.backend.follow_refreshes(refresh_ns, interval_ns);
    }

    /// Reports whether as many timed presents wait for the display as it holds at once.
    pub(crate) fn display_full(&self) -> bool {
        self.backend.display_full()
    }

    /// Returns how many presents the display holds at once.
    pub(crate) fn display_depth(&self) -> u32 {
        self.backend.display_depth()
    }

    /// Moves the numbers of presents and the times they were shown on the display into `out`, on
    /// the frame clock, oldest first.
    pub(crate) fn presented(&mut self, out: &mut Vec<(u64, u64)>) {
        self.backend.presented(out);
    }

    /// Returns the extent of the images, in texels.
    pub(crate) fn extent(&self) -> [u32; 2] {
        self.backend.extent()
    }

    /// Checks the acquired image at `slot` against `composed`, when [`VERIFY_VARIABLE`] asks for
    /// it.
    ///
    /// [`VERIFY_VARIABLE`]: crate::present::native::VERIFY_VARIABLE
    pub(crate) fn verify(&self, slot: u32, composed: &wgpu::Texture, size: [u32; 2]) {
        if !self.verify || !self.backend.readable() {
            return;
        }
        if let Some(held) = self.current(slot) {
            let size = Size::new(size[0] as i32, size[1] as i32);
            crate::present::native::verify::check(&self.gpu, &held.texture, slot, composed, size);
        }
    }

    /// Returns the view of the image at `slot`, made at its first acquisition.
    // Every acquisition asks for the presenter's one attachment format.
    fn view_of(
        &mut self,
        slot: u32,
        texture: &wgpu::Texture,
        view_format: wgpu::TextureFormat,
    ) -> wgpu::TextureView {
        let slot = slot as usize;
        if self.views.len() <= slot {
            self.views.resize(slot + 1, None);
        }
        self.views[slot]
            .get_or_insert_with(|| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(view_format),
                    ..Default::default()
                })
            })
            .clone()
    }

    /// Returns the image at `slot`, if it is acquired and not parked.
    fn current(&self, slot: u32) -> Option<&Held> {
        self.held
            .as_ref()
            .filter(|held| held.slot == slot && !held.released)
    }

    /// Makes every image owe the whole target after an answer of an out-of-date or lost chain.
    // Such a chain is made again, which discards every image's contents.
    fn forget_after(&mut self, answer: Acquisition) {
        if matches!(answer, Acquisition::Outdated | Acquisition::Lost) {
            self.views.clear();
            self.debts = ImageDebts::new(self.debts.len());
            self.presented = None;
        }
    }
}

/// Returns the rectangles of `damage` inside a frame of `frame` texels, or `None` for the whole
/// frame.
// A present without rectangles says nothing changed, so empty damage covers the whole frame.
fn changed_rects(damage: &DamageSet, frame: [u32; 2]) -> Option<Vec<Rect<i32, Device>>> {
    let whole = Rect::new(
        Point::new(0, 0),
        Size::new(frame[0] as i32, frame[1] as i32),
    );
    let rects = crate::frame::damage::rects(damage, whole);
    (!damage.is_full() && !rects.is_empty()).then_some(rects)
}
