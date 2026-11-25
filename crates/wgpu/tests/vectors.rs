//! Vector passes: rasterised by the path rasteriser into its scratch and composited at their place
//! in paint order.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{counter, draw, plain_renderer, present};
use iceblink::Renderer as _;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{FRAME_NO_CLIP, FramePath, MemoryReport, VectorBackend, VectorPlan, kurbo};
use iceblink_wgpu::vector::{
    ResourcedPass, ResourcedPlan, VectorError, VectorFrame, VectorRaster, VectorTarget,
};
use iceblink_wgpu::wgpu;

/// A rasteriser that fills each pass's raster region with its straight colour.
///
/// The colour of a pass is its entry in `colors`, or opaque blue past them. Each pass takes a round
/// of its own when `rounds` is set.
struct Solid {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    rounds: bool,
    colors: Vec<[u8; 4]>,
}

impl Solid {
    fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            rounds: false,
            colors: Vec::new(),
        }
    }
}

impl VectorRaster for Solid {
    fn backend(&self) -> VectorBackend {
        VectorBackend::Other
    }

    fn plan(&mut self, plan: &VectorPlan) -> ResourcedPlan {
        let mut resourced = ResourcedPlan::resourcing(plan);
        for (index, pass) in plan.passes.iter().enumerate() {
            resourced.passes.push(ResourcedPass {
                region: pass.region,
                raster_region: Rect::new(Point::new(0, 0), pass.region.size),
                target: VectorTarget(0),
                round: if self.rounds { index } else { 0 },
                items: pass.items.clone(),
                clip: pass.clip,
                instanced: pass.instanced,
                clamped: pass.clamped,
            });
        }
        resourced
    }

    fn clear_targets(&mut self, _plan: &ResourcedPlan, _round: usize) {}

    fn prepare(&mut self, frame: &mut VectorFrame<'_>) -> Result<(), VectorError> {
        for index in frame.passes() {
            let size = frame.plan.passes[index].raster_region.size;
            let color = self.colors.get(index).copied().unwrap_or([0, 0, 255, 255]);
            let bytes = color.repeat((size.width * size.height) as usize);
            frame.queue.write_texture(
                self.texture.as_image_copy(),
                &bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width as u32 * 4),
                    rows_per_image: Some(size.height as u32),
                },
                wgpu::Extent3d {
                    width: size.width as u32,
                    height: size.height as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
        Ok(())
    }

    fn view(&self, _target: VectorTarget) -> Option<&wgpu::TextureView> {
        Some(&self.view)
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport::default()
    }
}

fn path() -> FramePath {
    path_at(16)
}

fn path_at(x: i32) -> FramePath {
    FramePath {
        path: Arc::new(kurbo::BezPath::new()),
        transform: kurbo::Affine::IDENTITY,
        brush: kurbo::Affine::IDENTITY,
        fill: None,
        stroke: None,
        clips: Arc::from(Vec::new()),
        ink: Rect::new(Point::new(x, 16), Size::new(32, 32)),
    }
}

#[test]
fn a_pass_composites_between_the_quads_painted_before_and_after_it() {
    let mut renderer = plain_renderer();
    let raster = Solid::new(renderer.gpu().device());
    renderer.set_vector_raster(Box::new(raster), "solid");
    let mut builder = FrameBuilder::new();
    let red = builder.solid(opaque(255, 0, 0));
    let green = builder.solid(opaque(0, 255, 0));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), red));
    builder.path(path(), FRAME_NO_CLIP);
    builder.quad(filled(rect(40.0, 40.0, 8.0, 8.0), green));

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(20, 20),
        [0, 0, 255, 255],
        "the pass over the field"
    );
    assert_eq!(
        pixels.rgba(44, 44),
        [0, 255, 0, 255],
        "the later quad over the pass"
    );
    assert_eq!(pixels.rgba(100, 100), [255, 0, 0, 255]);
    assert_eq!(renderer.vector_status().name, "solid");
}

#[test]
fn a_pass_without_a_rasteriser_draws_nothing_and_is_counted() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    builder.path(path(), FRAME_NO_CLIP);

    let stats = draw(&mut renderer, &builder.finish()).stats().unwrap();

    assert_eq!(stats.vector_passes, 1);
    assert_eq!(stats.vector_passes_without_raster, 1);
    assert_eq!(
        renderer.read_presented().unwrap().rgba(20, 20),
        [0, 0, 0, 0]
    );
    assert_eq!(counter(&renderer, "composites_deferred"), 1);
}

#[test]
fn a_factory_builds_the_rasteriser_from_what_it_carries() {
    let mut renderer = plain_renderer();
    let built = Arc::new(AtomicU32::new(0));
    let calls = Arc::clone(&built);
    let colors = vec![[0, 255, 0, 255]];
    renderer.set_vector_factory(
        Box::new(move |gpu, _size| {
            calls.fetch_add(1, Ordering::Relaxed);
            let mut raster = Solid::new(gpu.device());
            raster.colors = colors.clone();
            Box::new(raster)
        }),
        VectorBackend::Other,
        "solid",
    );

    assert_eq!(
        built.load(Ordering::Relaxed),
        1,
        "a renderer with a target builds the rasteriser before its first frame"
    );

    let mut builder = FrameBuilder::new();
    builder.path(path(), FRAME_NO_CLIP);

    let stats = draw(&mut renderer, &builder.finish()).stats().unwrap();

    assert_eq!(built.load(Ordering::Relaxed), 1);
    assert_eq!(stats.vector_passes_without_raster, 0);
    assert_eq!(
        renderer.read_presented().unwrap().rgba(20, 20),
        [0, 255, 0, 255],
        "the pass in the colour the factory gave the rasteriser"
    );
}

#[test]
fn passes_in_two_rounds_share_one_scratch_and_both_draw() {
    let mut renderer = plain_renderer();
    let mut raster = Solid::new(renderer.gpu().device());
    raster.rounds = true;
    renderer.set_vector_raster(Box::new(raster), "solid");
    let mut builder = FrameBuilder::new();
    builder.path(path_at(16), FRAME_NO_CLIP);
    builder.path(path_at(64), FRAME_NO_CLIP);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(20, 20),
        [0, 0, 255, 255],
        "the first round's pass"
    );
    assert_eq!(
        pixels.rgba(70, 20),
        [0, 0, 255, 255],
        "the second round's pass"
    );
}

#[test]
fn each_damaged_rectangle_rasterises_the_rounds_again_from_the_first() {
    let mut renderer = plain_renderer();
    let mut raster = Solid::new(renderer.gpu().device());
    raster.rounds = true;
    raster.colors = vec![[255, 0, 0, 255], [0, 0, 255, 128]];
    renderer.set_vector_raster(Box::new(raster), "solid");
    let everywhere = Rect::new(Point::new(0, 0), Size::new(128, 128));
    let mut damage = common::build::area(0, 0, 32, 32);
    damage.absorb_set(&common::build::area(80, 80, 32, 32));
    let mut builder = FrameBuilder::new().damage(damage);
    for _ in 0..2 {
        let mut path = path();
        path.ink = everywhere;
        builder.path(path, FRAME_NO_CLIP);
    }

    let pixels = present(&mut renderer, &builder.finish());

    // Both passes rasterise into the same texels, so a rectangle that composited the first pass
    // from the second round's raster would show blue over blue.
    let blended = pixels.rgba(16, 16);
    assert!(
        blended[0] > 100 && blended[2] > 100,
        "half blue over red in the first rectangle: {blended:?}"
    );
    assert_eq!(pixels.rgba(96, 96), blended, "and the same in the second");
}
