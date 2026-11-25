//! Declared damage taken in place of derived damage.

use iceblink::damage::DamageSet;
use iceblink::{ClientId, CompositorEvent, OutputId};

use crate::output::Outputs;
use crate::output::declared::DeclaredCheck;

/// Diverts the damage derived for every output in `declared`, checking it against the
/// declaration when `verify` holds and dropping it otherwise.
pub(crate) fn divert(outputs: &mut Outputs, declared: &[(OutputId, DamageSet)], verify: bool) {
    for (output, damage) in declared {
        if let Some(output) = outputs.get_mut(output) {
            output.diverted = Some(verify.then(|| DeclaredCheck::new(*damage)));
        }
    }
}

/// Adds each declared set to its output's damage and ends the diversion.
///
/// When the derived damage was checked, it is added in place of the declared set. If a derived
/// rectangle reached outside the declaration, the derived set is also reported as a
/// `DamageUndeclared` event and counted in `violations`.
pub(crate) fn settle(
    outputs: &mut Outputs,
    declared: &[(OutputId, DamageSet)],
    client: ClientId,
    events: &mut Vec<CompositorEvent>,
    violations: &mut u64,
) {
    for (id, damage) in declared {
        let Some(output) = outputs.get_mut(id) else {
            continue;
        };
        let Some(check) = output.diverted.take().flatten() else {
            output.pending.absorb_set(damage);
            continue;
        };

        output.pending.absorb_set(&check.derived);
        if check.undeclared {
            *violations += 1;
            events.push(CompositorEvent::DamageUndeclared {
                client,
                output: *id,
                derived: check.derived,
            });
        }
    }
}
