//! Turning a shader description into a shader module.

use iceblink::shader::MAX_PARAMS_BYTES;
use iceblink::{ShaderDesc, ShaderMode};
use iceblink_wgsl::PREBUILT_FORMAT;

use crate::gpu::device::Gpu;
use crate::shader::layout::parse;

/// Checks that `desc` can be drawn.
///
/// A description can be drawn when:
///
/// 1. its mode is known,
/// 2. its parameters fit the block, and
/// 3. if it has parameters, the `Params` its WGSL declares fit in `params_size`.
///
/// Fails with a description of the first condition that does not hold.
pub fn check_shader(desc: &ShaderDesc) -> Result<(), String> {
    if iceblink_wgsl::epilogue(desc.mode).is_none() {
        return Err(format!("the shader mode {} is not known", desc.mode.0));
    }
    if desc.params_size as usize > MAX_PARAMS_BYTES {
        return Err(format!(
            "parameters are {} bytes and at most {MAX_PARAMS_BYTES} fit in the block",
            desc.params_size
        ));
    }
    let source = assemble(desc)?;
    let declared = parse(&source).get("Params").map_or(0, |layout| layout.size);
    // A shader without parameters is given a placeholder `Params`, which the block holds.
    if desc.params_size > 0 && declared > desc.params_size as usize {
        return Err(format!(
            "`Params` is {declared} bytes and the description gives {}",
            desc.params_size
        ));
    }
    Ok(())
}

/// Returns the whole WGSL module of `desc`.
pub(crate) fn assemble(desc: &ShaderDesc) -> Result<String, String> {
    iceblink_wgsl::effect(desc.mode, &desc.source.wgsl)
        .ok_or_else(|| format!("the shader mode {} is not known", desc.mode.0))
}

/// Compiles `desc`, from its prebuilt IR when the format is [`PREBUILT_FORMAT`].
///
/// A prebuilt shader of another format, or one whose bytes do not decode, is compiled from its
/// WGSL.
///
/// Fails when the description does not pass [`check_shader`].
pub(crate) fn compile(
    gpu: &Gpu,
    desc: &ShaderDesc,
    label: &str,
) -> Result<wgpu::ShaderModule, String> {
    check_shader(desc)?;
    let prebuilt = desc
        .source
        .prebuilt
        .as_ref()
        .filter(|prebuilt| &*prebuilt.format == PREBUILT_FORMAT)
        .and_then(|prebuilt| bincode::deserialize::<wgpu::naga::Module>(&prebuilt.bytes).ok());
    let source = match prebuilt {
        Some(module) => wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        None => wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(assemble(desc)?)),
    };
    Ok(gpu
        .device()
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source,
        }))
}

/// Reports whether shaders of `mode` draw a rectangle.
pub(crate) fn draws_rectangle(mode: ShaderMode) -> bool {
    iceblink_wgsl::is_rectangle(mode)
}
