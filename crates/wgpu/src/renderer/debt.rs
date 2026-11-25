//! Copying the kept target into an acquired image over the image's debt.

use iceblink::STALE_DAMAGE;
use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Point, Rect, Size};

use crate::gpu::timing::{PassQueries, TimedWork};
use crate::pipeline::kind::PipelineKind;
use crate::renderer::WgpuRenderer;

impl WgpuRenderer {
    /// Copies the kept target into `view` over `scissors`, and returns the draws and the pixels
    /// copied.
    pub(crate) fn blit(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        scissors: &[Rect<i32, Device>],
    ) -> (u32, u64) {
        let formats = self.presenter.formats();
        let kind = if formats.blit_undoes_srgb() {
            PipelineKind::BlitUndoSrgb
        } else {
            PipelineKind::Blit
        };
        let mut pipelines = self.pipelines.borrow_mut();
        let Some(pipeline) = pipelines.get(&self.gpu, kind, formats.present_attachment()) else {
            return (0, 0);
        };
        // An image that keeps its texels is copied only its debt. Every other image is copied
        // whole, and loading it would make wgpu clear it first anyway.
        let load = if self.presenter.keeps_contents() {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
        };
        let queries = self
            .timer
            .as_mut()
            .and_then(|timer| timer.pass(TimedWork::Blit));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("iceblink.present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: queries.as_ref().map(PassQueries::render),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.kept_binding, &[]);
        let mut drawn = 0;
        let mut copied = 0;
        for rect in scissors {
            let (width, height) = (
                rect.size.width.max(0) as u32,
                rect.size.height.max(0) as u32,
            );
            if width == 0 || height == 0 {
                continue;
            }
            pass.set_scissor_rect(
                rect.origin.x.max(0) as u32,
                rect.origin.y.max(0) as u32,
                width,
                height,
            );
            pass.draw(0..4, 0..1);
            drawn += 1;
            copied += u64::from(width) * u64::from(height);
        }
        (drawn, copied)
    }

    /// Returns the rectangles a copy into an image of `extent` with `debt` covers, cut to the kept
    /// target and the image.
    pub(crate) fn debt_scissors(
        &self,
        debt: &DamageSet<STALE_DAMAGE>,
        extent: [u32; 2],
    ) -> Vec<Rect<i32, Device>> {
        let image = Rect::new(
            Point::new(0, 0),
            Size::new(extent[0] as i32, extent[1] as i32),
        );
        let Some(whole) = self.kept.used().intersection(image) else {
            return Vec::new();
        };
        // An image that does not keep its texels owes the whole frame.
        if debt.is_full() || !self.presenter.keeps_contents() {
            return vec![whole];
        }
        debt.rects()
            .iter()
            .filter_map(|rect| rect.intersection(whole))
            .filter(|rect| !rect.is_empty())
            .collect()
    }
}
