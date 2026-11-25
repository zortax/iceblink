//! Random scenes played through two drives, whose images are compared after every step.

use crate::conformance::end_to_end::Pair;
use crate::conformance::end_to_end::drive::Drive;
use crate::conformance::end_to_end::scene::{Ids, Model, edit, scene};
use crate::golden::{Tolerance, difference};

/// How many scenes each check plays.
pub(super) const SEEDS: u64 = 8;

/// How many steps each scene plays after it is built.
pub(super) const STEPS: u32 = 24;

/// A drive and the ids of the scene it holds.
pub(super) struct Player {
    pub(super) drive: Drive,
    ids: Option<Ids>,
}

impl Player {
    /// Returns a player of `drive` that builds its scene on the first step.
    pub(super) fn new(drive: Drive) -> Self {
        Self { drive, ids: None }
    }

    /// Builds the scene on the first step and makes `edits` random edits on every later one,
    /// from the state of `model`.
    pub(super) fn step(&mut self, model: &mut Model, edits: u64) {
        let output = self.drive.output();
        let ids = &mut self.ids;
        self.drive.commit(|tx| match ids {
            None => *ids = Some(scene(tx, output, model)),
            Some(ids) => {
                for _ in 0..edits {
                    edit(tx, ids, model);
                }
            }
        });
    }
}

/// Plays random scenes through a drive of `first` and one of `second`, and compares their images
/// after every step within `tolerance`.
///
/// Each drive is given as a pair and whether it redraws the whole output. Nothing is played when
/// a target factory has no target.
///
/// # Panics
///
/// Panics naming `what` when the images differ by more than `tolerance`.
#[track_caller]
pub(super) fn play(
    first: (Pair<'_>, bool),
    second: (Pair<'_>, bool),
    tolerance: Tolerance,
    what: &str,
) {
    for seed in 0..SEEDS {
        let Some(drive) = Drive::new(first.0, first.1) else {
            return;
        };
        let mut a = Player::new(drive);
        let Some(drive) = Drive::new(second.0, second.1) else {
            return;
        };
        let mut b = Player::new(drive);
        let blend_modes = a.drive.blend_modes() & b.drive.blend_modes();
        let mut model = Model::new(seed, blend_modes);

        for step in 0..=STEPS {
            let edits = model.edits();
            let mut copy = model.clone();
            a.step(&mut model, edits);
            b.step(&mut copy, edits);

            let left = a.drive.draw();
            let right = b.drive.draw();
            let found = difference(&left, &right, tolerance).expect("the images have one size");
            assert!(
                found.pixels <= tolerance.max_pixels,
                "{what}, seed {seed}, step {step}: {} pixels differ by more than {}, the first at \
                 {:?}, by up to {}",
                found.pixels,
                tolerance.max_channel,
                found.first,
                found.max_channel
            );
        }
    }
}
