//! Applying a transaction to a copy of its client, and validating the state it leaves.
//!
//! Commands apply in order to a draft. Checks that concern one command, such as a stale id or a
//! splice past the end, run as the command applies, and the first command that fails one is
//! reported. Checks that concern the state the whole transaction leaves, such as the slots of
//! recorded items or objects still in use, run on the finished draft, and the failure attributed
//! to the earliest command is reported. A commit that passes both replaces the client with the
//! draft.

mod apply;
mod check;
mod draft;
mod finished;
mod precedence;

use std::collections::{HashMap, HashSet};

use iceblink::FontRegistry;
use iceblink::{
    AnyId, ClientId, CommitCounters, CommitError, OrderId, OutputId, RejectReason, TransactionParts,
};

use crate::naive::scene::{Client, Scene};

use self::draft::Draft;

/// The result of a commit that passed every check.
pub(crate) struct Committed {
    pub(crate) client: ClientId,
    pub(crate) state: Client,
    pub(crate) roots: HashMap<OutputId, (ClientId, OrderId)>,
    /// The objects the commit removed, whose ids go back to the client's id space.
    pub(crate) removed: Vec<AnyId>,
    pub(crate) counters: CommitCounters,
}

/// The compositor state a commit reads besides its client.
pub(crate) struct Context<'a> {
    pub(crate) outputs: &'a HashSet<OutputId>,
    pub(crate) fonts: &'a dyn FontRegistry,
    /// The last image version handed out, unique across every image.
    pub(crate) versions: &'a mut u64,
}

/// Applies `parts` to a copy of its client's state and validates the result.
///
/// # Errors
///
/// Fails with `StaleId` when the client is unknown or `parts` carries another id space.
/// Otherwise fails with the rule the first failing command breaks, or with the broken rule of the
/// finished state that is attributed to the earliest command.
pub(crate) fn commit(
    scene: &Scene,
    parts: &mut TransactionParts,
    context: Context<'_>,
) -> Result<Committed, CommitError> {
    let client_id = parts.ids.client();
    let Some(client) = scene.clients.get(&client_id) else {
        return Err(rejected(RejectReason::StaleId, None));
    };

    if !client.ids.same(&parts.ids) {
        return Err(rejected(RejectReason::StaleId, None));
    }

    let mut draft = Draft::new(client_id, client.clone(), scene.roots.clone(), context);
    let commands = std::mem::take(&mut parts.commands);
    let mut chunks: Vec<_> = std::mem::take(&mut parts.chunks)
        .into_iter()
        .map(Some)
        .collect();

    for (index, command) in commands.into_iter().enumerate() {
        let index = index as u32;
        apply::command(&mut draft, &mut chunks, &parts.entries, command, index)
            .map_err(|reason| rejected(reason, Some(index)))?;
    }

    finished::check(&draft)?;

    Ok(draft.into_committed())
}

pub(crate) fn rejected(reason: RejectReason, command: Option<u32>) -> CommitError {
    CommitError { reason, command }
}
