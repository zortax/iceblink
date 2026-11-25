//! Acquiring an image from the native chain or from wgpu's swap chain.

use iceblink::{Acquired, BufferAge};

use super::SurfacePresenter;
use crate::present::acquisition::Acquisition;

impl SurfacePresenter {
    /// Acquires the next image, configuring the surface first if a configuration is owed.
    pub(super) fn acquire_image(&mut self) -> Acquired {
        // A configuration waits for the device to go idle, which then overlaps the frame
        // composed already.
        self.apply();
        let asked = web_time::Instant::now();
        let taken = match self.faults.take() {
            Some(injected) if !injected.presents() => Err(injected),
            _ => self.take_image(),
        };
        self.block = asked.elapsed();
        let loss = self.gpu.loss();
        match taken {
            Ok((image, answer)) => {
                if answer.reconfigures() {
                    self.reconfigure = true;
                }
                loss.note_acquisition_succeeded();
                let age = if self.native.is_some() {
                    BufferAge::Kept
                } else {
                    BufferAge::Unknown
                };
                Acquired::Image { image, age }
            }
            Err(answer) => {
                self.note_failure(answer);
                Acquired::Skipped(answer.skip())
            }
        }
    }

    /// Records what an answer without an image means for the surface and the device.
    pub(super) fn note_failure(&mut self, answer: Acquisition) {
        let loss = self.gpu.loss();
        if answer == Acquisition::Validation && loss.note_validation_failure() {
            loss.report(
                wgpu::DeviceLostReason::Unknown,
                "acquisition failed validation repeatedly",
            );
        }
        if answer.reconfigures() {
            self.reconfigure = true;
        }
        if answer == Acquisition::Lost {
            self.configured = false;
        }
    }

    /// Returns the acquired image and the answer that gave it.
    fn take_image(&mut self) -> Result<(u32, Acquisition), Acquisition> {
        let view_format = self.formats.present_attachment();
        if let Some(native) = &mut self.native {
            let (slot, suboptimal) = native.acquire(view_format)?;
            let answer = if suboptimal {
                Acquisition::Suboptimal
            } else {
                Acquisition::Success
            };
            return Ok((slot, answer));
        }
        let current = self.surface.get_current_texture();
        let answer = Acquisition::classify(&current);
        let texture = match current {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            _ => return Err(answer),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(view_format),
            ..Default::default()
        });
        self.sequence = self.sequence.wrapping_add(1);
        self.acquired = Some((self.sequence, texture, view));
        Ok((self.sequence, answer))
    }
}
