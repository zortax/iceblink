//! Parses every shader module at build time and writes its text and naga IR to `OUT_DIR`, with the
//! fingerprint of the pipeline set, and sets the `vulkan_hal` cfg.
//!
//! A module that does not parse fails the build.

use std::path::{Path, PathBuf};

/// Declares `vulkan_hal` and sets it on the targets where wgpu links a Vulkan backend.
fn declare_vulkan_hal() {
    // The targets on which wgpu-core 30.0.2 depends on its Vulkan backend crate.
    println!("cargo::rustc-check-cfg=cfg(vulkan_hal)");
    let target =
        std::env::var("CARGO_CFG_TARGET_OS").expect("cargo names the target it builds for");
    if matches!(target.as_str(), "windows" | "linux" | "android" | "freebsd") {
        println!("cargo::rustc-cfg=vulkan_hal");
    }
}

fn main() {
    declare_vulkan_hal();

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo names an output directory"));
    let mut pipelines = Fingerprint::default();
    write_modules(&out, &mut pipelines);
    add_pipeline_code(&mut pipelines);

    write(
        &out.join("pipelines.rs"),
        format!(
            "/// The fingerprint of the renderer's pipeline set, which names its cache file.\n\
             const PIPELINES: u64 = {:#018x};\n",
            pipelines.0
        )
        .as_bytes(),
    );
}

/// Writes the text and naga IR of every shader module to `out`, and adds each module's name and
/// IR to `pipelines`.
fn write_modules(out: &Path, pipelines: &mut Fingerprint) {
    for (name, _) in iceblink_wgsl::MODULES {
        let source = iceblink_wgsl::module(name).expect("a listed module assembles");

        let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| {
            panic!("{name}: {}", error.emit_to_string(&source));
        });
        let ir = bincode::serialize(&module)
            .unwrap_or_else(|error| panic!("{name}: the IR would not encode: {error}"));

        write(&out.join(format!("{name}.wgsl")), source.as_bytes());
        write(&out.join(format!("{name}.ir")), &ir);
        pipelines.add(name.as_bytes());
        pipelines.add(&ir);
    }
}

/// Adds the text of the code that describes the pipelines, `src/pipeline`, to `pipelines`.
// Without `rerun-if-changed`, cargo runs this script again whenever a file of the package changes.
fn add_pipeline_code(pipelines: &mut Fingerprint) {
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo names the package directory"),
    );
    for file in sources(&manifest.join("src").join("pipeline")) {
        let text =
            std::fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        pipelines.add(&text);
    }
}

/// Returns the Rust sources under `directory`, in a fixed order.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a directory entry reads").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// A 64-bit FNV-1a hash, built up part by part.
struct Fingerprint(u64);

impl Default for Fingerprint {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fingerprint {
    fn add(&mut self, bytes: &[u8]) {
        let length = (bytes.len() as u64).to_le_bytes();
        for &byte in bytes.iter().chain(&length) {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// Writes one generated file.
fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}
