//! The content each pass key was issued for.

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::PassKey;
use iceblink::kurbo;

/// The digest of the content each pass key was first issued for, with the outlines it names.
#[derive(Clone, Debug, Default)]
pub(crate) struct Issued {
    digests: HashMap<PassKey, (u64, Vec<Arc<kurbo::BezPath>>)>,
}

impl Issued {
    /// Records `digest` for `key`, holding `outlines` so no other outline takes their addresses,
    /// and returns the digest the key was issued for before, if it differs.
    pub(crate) fn issue(
        &mut self,
        key: PassKey,
        digest: u64,
        outlines: impl FnOnce() -> Vec<Arc<kurbo::BezPath>>,
    ) -> Option<u64> {
        match self.digests.get(&key) {
            Some(&(held, _)) => (held != digest).then_some(held),
            None => {
                self.digests.insert(key, (digest, outlines()));
                None
            }
        }
    }
}
