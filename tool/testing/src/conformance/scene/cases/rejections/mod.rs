//! Commit validation: every rejection reason, atomic rejection, and the ids a rejection frees.

mod animations;
mod items;
mod paints;
mod references;
mod resources;
mod trees;

use iceblink::{Color, CommitError, Paint, PaintId, PaintRef, Quad, RejectReason, Transaction};

use crate::conformance::scene::build::{key, quad, record_quads};
use crate::conformance::scene::stage::Stage;

pub use self::animations::unsupported_animations;
pub use self::items::invalid_items;
pub use self::paints::paint_kinds;
pub use self::references::{in_use, stale_ids, unknown_chunks, unknown_outputs};
pub use self::resources::{invalid_shaders, invalid_writes};
pub use self::trees::{cycles, order_conflicts, splices_out_of_range};

const RED: [u8; 4] = [255, 0, 0, 255];

/// Asserts that committing `tx` is rejected for `reason`, that the rejection changed nothing
/// drawn, and that it freed the ids the transaction minted.
#[track_caller]
fn rejects(stage: &mut Stage, mut tx: Transaction, reason: RejectReason, case: &str) {
    // A visible quad rides along, so a partly applied commit would draw it.
    record_quads(
        &mut tx,
        key(9_000),
        stage.props(),
        &[([0.0, 0.0, 8.0, 8.0], RED)],
    );
    let witness = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));

    let result = stage.try_commit(tx);
    match result {
        Err(CommitError { reason: found, .. }) if found == reason => {}
        other => panic!("{case}: expected a rejection for {reason:?}, got {other:?}"),
    }

    let again: PaintId = stage.ids.mint();
    assert!(
        again.slot() == witness.slot() && again.generation() > witness.generation(),
        "{case}: the rejection frees the ids it minted with a later generation, but the next \
         paint is {again:?} after {witness:?}"
    );
    let picture = stage.redraw();
    assert!(
        picture.quads_of(RED).is_empty(),
        "{case}: a rejected commit drew part of its transaction"
    );
}

/// A quad of 8 by 8 at the origin filled with `fill`.
fn quad_with(fill: PaintRef) -> Quad {
    let mut item = quad([0.0, 0.0, 8.0, 8.0], RED);
    item.fill = fill;
    item
}
