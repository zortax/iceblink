//! What a golden comparison does with its outcome.

use std::fs;
use std::path::Path;

use crate::golden::bless::{BLESS, is_blessing};

/// The outcome of comparing a rendering with its golden file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The rendering matched the golden.
    Matched,
    /// The golden file does not exist.
    Missing,
    /// The rendering differed, with a report of the difference.
    Differed(String),
}

/// Passes on a match, and otherwise fails the test, writing the rendering with `write` first when
/// blessing.
///
/// # Panics
///
/// Panics unless `outcome` is a match, and when `write` fails.
#[track_caller]
pub(crate) fn settle(
    path: &Path,
    outcome: Outcome,
    write: impl FnOnce(&Path) -> std::io::Result<()>,
) {
    let blessing = is_blessing();
    match (outcome, blessing) {
        (Outcome::Matched, _) => {}
        (Outcome::Missing, false) => panic!(
            "no golden at {}. Run with {BLESS}=1 to write it, and read it before committing it.",
            path.display()
        ),
        (Outcome::Missing, true) => {
            create(path, write);
            panic!(
                "wrote a new golden at {}. Read it, then run without {BLESS} to check it.",
                path.display()
            );
        }
        (Outcome::Differed(report), false) => panic!(
            "{} does not match the rendering.\n{report}\nRun with {BLESS}=1 to accept the new \
             rendering.",
            path.display()
        ),
        (Outcome::Differed(report), true) => {
            create(path, write);
            panic!(
                "rewrote {}.\n{report}\nRun without {BLESS} to check the new rendering.",
                path.display()
            );
        }
    }
}

#[track_caller]
fn create(path: &Path, write: impl FnOnce(&Path) -> std::io::Result<()>) {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)
            .unwrap_or_else(|error| panic!("could not create {}: {error}", directory.display()));
    }
    write(path).unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));
}
