//! The file a cache is kept in: its name, its directory, and writes that never shrink it.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::pipeline_cache::fingerprint::fingerprint;

/// The environment variable that holds the directory caches are kept in.
pub const DIRECTORY_VARIABLE: &str = "ICEBLINK_CACHE_DIR";

/// How many files of one adapter a directory keeps.
const KEPT: usize = 4;

/// Returns the directory caches are kept in by default.
///
/// The directory is the first of these whose variable is set to valid Unicode:
///
/// 1. The directory in [`DIRECTORY_VARIABLE`].
/// 2. `iceblink/pipelines` under `XDG_CACHE_HOME`.
/// 3. `.cache/iceblink/pipelines` under `HOME`.
///
/// Returns `None` when none of the three is set.
pub fn default_directory() -> Option<PathBuf> {
    if let Ok(named) = std::env::var(DIRECTORY_VARIABLE) {
        return Some(PathBuf::from(named));
    }
    let home = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok()?;
    Some(home.join("iceblink").join("pipelines"))
}

/// The file of one pipeline set on one adapter and driver.
#[derive(Debug)]
pub(crate) struct CacheFile {
    path: PathBuf,
    /// The name prefix shared by every file of the same adapter.
    adapter: String,
    /// Whether the file holds data the driver refused, which any write replaces.
    refused: Cell<bool>,
}

impl CacheFile {
    /// Returns the file in `directory` of pipeline set `name` with fingerprint `pipelines`,
    /// compiled on the adapter and driver `adapter` describes.
    pub(crate) fn new(
        directory: PathBuf,
        name: &str,
        adapter: &wgpu::AdapterInfo,
        pipelines: u64,
    ) -> Self {
        let driver = fingerprint(&[
            format!("{:?}", adapter.backend).as_bytes(),
            adapter.driver.as_bytes(),
            adapter.driver_info.as_bytes(),
            &pipelines.to_le_bytes(),
        ]);
        let prefix = format!("{name}-{:04x}{:04x}-", adapter.vendor, adapter.device);
        Self {
            path: directory.join(format!("{prefix}{driver:016x}")),
            adapter: prefix,
            refused: Cell::new(false),
        }
    }

    /// Returns the file's contents, or `None` if it cannot be read.
    pub(crate) fn read(&self) -> Option<Vec<u8>> {
        std::fs::read(&self.path).ok()
    }

    /// Notes whether the driver refused the file's contents.
    ///
    /// A file the driver accepted counts as the most recently used of its adapter.
    pub(crate) fn loaded(&self, refused: bool) {
        self.refused.set(refused);
        if !refused {
            // A launch that compiles nothing writes nothing, and the file would age out.
            let _ = std::fs::File::options()
                .write(true)
                .open(&self.path)
                .and_then(|file| file.set_modified(SystemTime::now()));
        }
    }

    /// Replaces the file with `data` if it holds more than the file, or the file was refused,
    /// and then removes the adapter's files beyond the most recent four.
    pub(crate) fn write(&self, data: &[u8]) {
        let held = if self.refused.get() {
            0
        } else {
            std::fs::metadata(&self.path).map_or(0, |metadata| metadata.len())
        };
        if data.len() as u64 <= held {
            return;
        }
        let Some(directory) = self.path.parent() else {
            return;
        };
        if let Err(error) = std::fs::create_dir_all(directory) {
            tracing::debug!(?error, "the pipeline cache directory could not be created");
            return;
        }
        // Unique to this write, because processes and threads share the file.
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let write = WRITES.fetch_add(1, Ordering::Relaxed);
        let staged = self
            .path
            .with_extension(format!("{}.{write}.tmp", std::process::id()));
        match std::fs::write(&staged, data).and_then(|()| std::fs::rename(&staged, &self.path)) {
            Ok(()) => {
                self.refused.set(false);
                prune(directory, &self.adapter);
            }
            Err(error) => {
                let _ = std::fs::remove_file(&staged);
                tracing::debug!(?error, "the pipeline cache could not be written");
            }
        }
    }
}

/// Removes the files in `directory` whose names start with `adapter`, except the four used most
/// recently.
fn prune(directory: &Path, adapter: &str) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(adapter) && !name.ends_with(".tmp"))
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, path) in files.into_iter().skip(KEPT) {
        let _ = std::fs::remove_file(path);
    }
}
