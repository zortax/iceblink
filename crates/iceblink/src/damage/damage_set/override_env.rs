//! The environment switch that forces full damage on every frame.

use std::env;
use std::sync::OnceLock;

const VARIABLE: &str = "ICEBLINK_FULL_DAMAGE";

/// Reports whether the environment variable `ICEBLINK_FULL_DAMAGE` is set to `1`.
///
/// When it is, [`DamageSet::for_frame`](crate::damage::DamageSet::for_frame) starts every frame
/// full. The environment is read on the first call and the answer is kept.
pub fn full_damage_forced() -> bool {
    static FORCED: OnceLock<bool> = OnceLock::new();
    *FORCED.get_or_init(|| env::var(VARIABLE).is_ok_and(|value| value == "1"))
}
