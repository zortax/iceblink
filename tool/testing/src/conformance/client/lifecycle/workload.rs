//! The fixed workload two stepped runs are compared on.

use iceblink::{ChunkKey, GesturePhase, OrderEntry, ScrollPhysics};

use crate::conformance::client::Rig;
use crate::conformance::client::stage::{ClientStage, quads};

/// Runs a fixed workload of commits and, where the compositor routes gestures, a fling, and
/// returns every frame drawn and every event queued, one line each.
///
/// The workload depends only on the step clocks. The lines leave out client and commit ids.
pub fn transcript(rig: &Rig<'_>) -> Vec<String> {
    let mut stage = ClientStage::new(rig);
    let scrolls = stage.driver.capabilities().scroll_input;
    let node = stage.list(2000.0, ScrollPhysics::Kinetic);
    let to = stage.to(node);

    let mut lines = Vec::new();
    for index in 0..24_u32 {
        if index % 6 == 0 {
            let mut tx = stage.tx();
            let key = ChunkKey::new(10 + u64::from(index), 0);
            let at = 8.0 * (index / 6) as f32;
            quads(&mut tx, key, stage.space, &[[at, 180.0, 6.0, 6.0]]);
            stage.push(&mut tx, OrderEntry::Chunk(key));
            stage.driver.commit(tx);
        }
        if scrolls && index < 4 {
            let phase = match index {
                0 => GesturePhase::Begin,
                3 => GesturePhase::End {
                    velocity: [0.0, 2400.0],
                },
                _ => GesturePhase::Update,
            };
            stage.gesture([100.0, 100.0], [0.0, 12.0], phase, to);
        }
        stage.step();
        if index % 6 == 0 {
            // A second commit at the same instant is built by a second step there.
            let mut tx = stage.tx();
            let key = ChunkKey::new(100 + u64::from(index), 0);
            quads(&mut tx, key, stage.space, &[[100.0, 180.0, 6.0, 6.0]]);
            stage.push(&mut tx, OrderEntry::Chunk(key));
            stage.driver.commit(tx);
            stage.step_again();
        }
        lines.extend(
            stage
                .events()
                .iter()
                .map(|event| format!("step {index}: {}", anonymous(&format!("{event:?}")))),
        );
    }

    let log = stage.log();
    lines.extend(log.draws.iter().map(|frame| format!("{frame:?}")));
    lines.push(format!("woken {}", stage.woken()));
    lines
}

/// Returns `line` with the numbers of its client and commit ids left out.
fn anonymous(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    // Drivers number clients and commits apart.
    while let Some(at) = ["ClientId(", "CommitId("]
        .iter()
        .filter_map(|name| rest.find(name).map(|at| at + name.len()))
        .min()
    {
        out.push_str(&rest[..at]);
        rest = rest[at..].trim_start_matches(|c: char| c.is_ascii_digit());
    }
    out.push_str(rest);
    out
}
