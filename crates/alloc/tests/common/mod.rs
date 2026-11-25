//! Builders and the proptest configuration shared by the arena and atlas tests.

// Every test binary compiles this module and uses a subset of it.
#![allow(dead_code)]

pub(crate) mod arena;
pub(crate) mod atlas;

use proptest::test_runner::{Config, FileFailurePersistence};

/// The proptest configuration, with fewer cases and no persistence under miri.
///
/// Failures persist beside each test file with the extension `regressions`.
pub(crate) fn config() -> Config {
    let mut config = Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("regressions"))),
        ..Config::default()
    };
    if cfg!(miri) {
        config.failure_persistence = None;
        config.cases = config.cases.min(8);
    }
    config
}
