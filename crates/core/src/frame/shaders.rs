//! The shader work of a frame.

use std::sync::Arc;

use iceblink::{Frame, ShaderOp};

use crate::output::renderer::RendererState;
use crate::tables::FrameTables;

/// Tells the renderer to destroy the removed shaders it holds and to create the shaders the
/// frame draws with that it does not hold.
pub(crate) fn work(
    tables: &FrameTables,
    renderer: &mut RendererState,
    drawn: &[u32],
    frame: &mut Frame,
) {
    let dead: Vec<u32> = renderer
        .shaders
        .iter()
        .copied()
        .filter(|slot| !tables.shaders.contains_key(slot))
        .collect();
    for shader in dead {
        renderer.shaders.remove(&shader);
        frame.resources.shaders.push(ShaderOp::Destroy { shader });
    }

    for &shader in drawn {
        let Some(desc) = tables.shaders.get(&shader) else {
            continue;
        };
        if renderer.shaders.insert(shader) {
            frame.resources.shaders.push(ShaderOp::Create {
                shader,
                desc: Arc::clone(desc),
            });
        }
    }
}
