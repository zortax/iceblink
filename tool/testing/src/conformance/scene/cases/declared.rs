//! Declared damage and hints, checked in verification mode.

use iceblink::damage::DamageSet;
use iceblink::{Built, CompositorEvent};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{device, key, pixels, quad, record_quads};
use crate::conformance::scene::stage::{SIDE, Stage, area, covers};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Drains the compositor's queued events.
fn events(stage: &mut Stage) -> Vec<CompositorEvent> {
    let mut events = Vec::new();
    stage.compositor.drain_events(&mut events);
    events
}

/// The damage of a transaction that declared its damage covers the change, also when the
/// declaration misses part of it, and a precise compositor reports and counts only a declaration
/// that misses something.
pub fn declared_damage(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.compositor.set_verification(true);
    stage.add_quads(key(1), &[([10.0, 10.0, 20.0, 20.0], RED)]);
    stage.draw();
    events(&mut stage);

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([10.0, 10.0, 20.0, 20.0], BLUE)],
    );
    let mut declared = DamageSet::new();
    declared.absorb(device(10, 10, 20, 20));
    tx.declare_damage(stage.output, declared);
    stage.commit(tx);
    assert_eq!(stage.draw(), Built::Frame);
    assert!(covers(stage.damage(), pixels([10.0, 10.0, 20.0, 20.0])));
    let found = events(&mut stage);
    // A compositor that is not precise derives wider damage than the precision rules give, so
    // only a precise one is held to its reports.
    let precise = stage.precise("declared_damage");
    assert!(
        !precise
            || !found
                .iter()
                .any(|event| matches!(event, CompositorEvent::DamageUndeclared { .. })),
        "a declaration that holds the change is not reported: {found:?}"
    );
    let violations = stage.compositor.counters().damage_violations;

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([60.0, 60.0, 20.0, 20.0], RED)],
    );
    let mut declared = DamageSet::new();
    declared.absorb(device(10, 10, 20, 20));
    tx.declare_damage(stage.output, declared);
    stage.commit(tx);
    assert_eq!(stage.draw(), Built::Frame);
    assert!(
        covers(stage.damage(), pixels([60.0, 60.0, 20.0, 20.0])),
        "verification damages what the declaration missed: {:?}",
        stage.damage()
    );
    if !precise {
        return;
    }
    let found = events(&mut stage);
    assert!(
        found
            .iter()
            .any(|event| matches!(event, CompositorEvent::DamageUndeclared { .. })),
        "a declaration that misses the change is reported: {found:?}"
    );
    assert_eq!(
        stage.compositor.counters().damage_violations,
        violations + 1,
        "the violation is counted"
    );
}

/// In verification mode, a transaction that declared more damage than it changed is damaged as
/// if it declared none.
pub fn verified_declarations(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.compositor.set_verification(true);
    stage.add_quads(key(1), &[([10.0, 10.0, 20.0, 20.0], RED)]);
    stage.draw();

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([10.0, 10.0, 20.0, 20.0], BLUE)],
    );
    let mut declared = DamageSet::new();
    declared.absorb(device(0, 0, SIDE as i32, SIDE as i32));
    tx.declare_damage(stage.output, declared);
    stage.commit(tx);
    assert_eq!(stage.draw(), Built::Frame);
    assert!(covers(stage.damage(), pixels([10.0, 10.0, 20.0, 20.0])));
    if !stage.precise("verified_declarations") {
        return;
    }
    let damaged = area(stage.damage());
    assert!(
        damaged <= 21 * 21,
        "the declaration of the whole output is ignored, and {damaged} pixels are damaged"
    );
}

/// A recording that changes something outside its hint is damaged in full, and a precise
/// compositor reports and counts it.
pub fn hint_violations(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.compositor.set_verification(true);
    stage.add_quads(
        key(1),
        &[
            ([10.0, 10.0, 20.0, 20.0], RED),
            ([70.0, 70.0, 20.0, 20.0], RED),
        ],
    );
    stage.draw();
    events(&mut stage);
    let violations = stage.compositor.counters().hint_violations;

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    chunk.quad(quad([10.0, 10.0, 20.0, 20.0], BLUE));
    chunk.quad(quad([70.0, 70.0, 20.0, 20.0], BLUE));
    chunk.hint([10.0, 10.0, 20.0, 20.0]);
    stage.commit(tx);
    assert_eq!(stage.draw(), Built::Frame);
    assert!(
        covers(stage.damage(), pixels([70.0, 70.0, 20.0, 20.0])),
        "verification damages the change outside the hint: {:?}",
        stage.damage()
    );
    if !stage.precise("hint_violations") {
        return;
    }
    let found = events(&mut stage);
    assert!(
        found
            .iter()
            .any(|event| matches!(event, CompositorEvent::HintViolated { .. })),
        "a hint that misses the change is reported: {found:?}"
    );
    assert_eq!(stage.compositor.counters().hint_violations, violations + 1);
}
