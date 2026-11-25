//! The switch for checks that cost something.

use std::sync::OnceLock;

/// Reports whether the environment variable `ICEBLINK_INVARIANTS` asks for costly checks.
///
/// The variable is read on the first call and the answer is kept. Any value other than empty or
/// `0` switches the checks on.
pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("ICEBLINK_INVARIANTS").is_ok_and(|value| !value.is_empty() && value != "0")
    })
}

/// Returns the most objects one draw order may hold before the order check gives up.
///
/// The environment variable `ICEBLINK_INVARIANTS_ORDER_MAX` sets it, read on the first call. It
/// defaults to [`DEFAULT_MAX`](crate::plan::order::sweep::DEFAULT_MAX).
pub(crate) fn order_max() -> usize {
    static MAX: OnceLock<usize> = OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("ICEBLINK_INVARIANTS_ORDER_MAX")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(crate::plan::order::sweep::DEFAULT_MAX)
    })
}
