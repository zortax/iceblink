//! Drawing a frame through a rasteriser, with the renderer's composites done on the host.

use iceblink::DeviceRect;
use iceblink::geom::{Point, Rect};
use iceblink_wgpu::vector::{
    ResourcedPass, ResourcedPlan, VectorError, VectorRaster, VectorTarget,
};

use crate::gpu::TestDevice;
use crate::vector::canvas::Canvas;
use crate::vector::readback;
use crate::vector::scene::Scene;

/// What drawing one frame came to.
pub struct Drawn {
    /// The output after every pass was composited over a black ground.
    pub canvas: Canvas,
    /// The first error any round's preparation reported, or `Ok`.
    pub outcome: Result<(), VectorError>,
    /// The resourced plan.
    pub plan: ResourcedPlan,
}

/// Draws `scene` through `raster` over opaque black.
pub fn draw(gpu: &TestDevice, raster: &mut dyn VectorRaster, scene: &Scene) -> Drawn {
    draw_on(gpu, raster, scene, [0, 0, 0, 255], 1)
}

/// Draws `scene` through `raster`, preparing every round twice, as a frame of two draw
/// rectangles does.
pub fn draw_twice(gpu: &TestDevice, raster: &mut dyn VectorRaster, scene: &Scene) -> Drawn {
    draw_on(gpu, raster, scene, [0, 0, 0, 255], 2)
}

/// Draws `scene` through `raster` over `background`, preparing every round `times` times.
///
/// Every round is cleared and prepared, then its targets are read back and its passes are
/// composited in plan order, as the renderer does. A pass at or past the `prepared` count of an
/// out-of-capacity error is left out.
pub fn draw_on(
    gpu: &TestDevice,
    raster: &mut dyn VectorRaster,
    scene: &Scene,
    background: [u8; 4],
    times: usize,
) -> Drawn {
    let plan = raster.plan(&scene.plan);
    let mut canvas = Canvas::filled(scene.side, background);
    let mut outcome = Ok(());
    let mut targets = Vec::new();
    for round in 0..plan.rounds() {
        for _ in 0..times {
            raster.clear_targets(&plan, round);
            let mut frame = scene.frame(&plan, round, gpu);
            outcome = outcome.and(raster.prepare(&mut frame));
        }

        plan.targets_of(round, &mut targets);
        let layers = read_layers(gpu, raster, &plan, round, &targets);
        let prepared = match &outcome {
            Err(VectorError::OutOfCapacity { prepared, .. }) => *prepared,
            _ => usize::MAX,
        };
        for index in plan.round(round).filter(|&index| index < prepared) {
            let pass = &plan.passes[index];
            let Some(layer) = layers.iter().find(|layer| layer.target == pass.target) else {
                continue;
            };
            composite(&mut canvas, &plan, pass, &layer.texels, layer.width);
        }
    }

    Drawn {
        canvas,
        outcome,
        plan,
    }
}

/// The texels of one scratch target, read back to the host.
struct Layer {
    target: VectorTarget,
    /// The width of the read area, in texels.
    width: u32,
    texels: Vec<[u8; 4]>,
}

/// Reads back the area of each of `targets` that the passes of `round` rasterise into.
fn read_layers(
    gpu: &TestDevice,
    raster: &dyn VectorRaster,
    plan: &ResourcedPlan,
    round: usize,
    targets: &[VectorTarget],
) -> Vec<Layer> {
    targets
        .iter()
        .map(|&target| {
            let passes = plan.passes[plan.round(round)]
                .iter()
                .filter(|pass| pass.target == target);
            let width = passes.clone().map(|pass| pass.raster_region.right()).max();
            let height = passes.map(|pass| pass.raster_region.bottom()).max();
            let view = raster.view(target).expect("a target the rasteriser placed");
            let width = width.unwrap_or(1).max(1) as u32;
            Layer {
                target,
                width,
                texels: readback::layer(gpu, view, width, height.unwrap_or(1) as u32),
            }
        })
        .collect()
}

/// Composites `pass` from the layer `texels`, `width` texels wide, onto `canvas`.
///
/// An instanced pass is composited one path ink at a time, and any other pass over its region.
fn composite(
    canvas: &mut Canvas,
    plan: &ResourcedPlan,
    pass: &ResourcedPass,
    texels: &[[u8; 4]],
    width: u32,
) {
    let size = pass.region.size;
    let mut copy = |relative: DeviceRect| {
        let (left, top) = (relative.left().max(0), relative.top().max(0));
        let (right, bottom) = (
            relative.right().min(size.width),
            relative.bottom().min(size.height),
        );
        for y in top..bottom {
            for x in left..right {
                let (column, row) = (
                    pass.raster_region.origin.x + x,
                    pass.raster_region.origin.y + y,
                );
                let at = row as usize * width as usize + column as usize;
                let inside = (column as u32) < width;
                let texel = texels.get(at).copied().filter(|_| inside).unwrap_or([0; 4]);
                canvas.blend(pass.region.origin.x + x, pass.region.origin.y + y, texel);
            }
        }
    };

    if pass.instanced {
        for planned in plan.items_of(pass) {
            copy(planned.ink);
        }
    } else {
        copy(Rect::new(Point::new(0, 0), pass.region.size));
    }
}
