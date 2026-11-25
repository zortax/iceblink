//! The present suite: the renderer contract of `draw`, `present_held` and the outcomes, run on
//! any renderer and presenter pair.
//!
//! A case draws frames built by hand on a [`PresentRig`] from the factory, in the presenter mode
//! the case names. Outcomes a renderer gives only under platform conditions, such as a held
//! present, an acquisition that times out or a lost device, are provoked through the rig's hooks.
//! A case whose hook the rig cannot provide, or whose mode the factory does not build, is skipped
//! with a line on standard error.
//!
//! [`present_suite!`](crate::present_suite) declares one test per case, and [`run_all`] runs every
//! case in one test.

mod cases;
mod stand;

use iceblink::{PresentMode, Renderer};

use crate::conformance::Case;
use crate::golden::RgbaImage;

pub use self::cases::{
    acquire_block, buffer_age, device_loss, held_frames, kept_target_age, timed_out_acquisitions,
    unconfigured, undamaged_frames,
};

/// A renderer under test and the hooks that provoke the outcomes of platform conditions.
///
/// The renderer is not configured yet. Every hook returns whether the rig provides it.
pub trait PresentRig {
    /// The renderer, which owns the presenter under test.
    fn renderer(&mut self) -> &mut dyn Renderer;

    /// Reads back the image presented last, or returns `None` before the first present.
    fn presented(&mut self) -> Option<RgbaImage>;

    /// Makes every present wait for the platform while `held` is true, so that a composed frame
    /// is answered with `Skipped(Held)`.
    fn hold_presents(&mut self, held: bool) -> bool;

    /// Makes the next `times` acquisitions time out.
    fn time_out_acquisitions(&mut self, times: u32) -> bool;

    /// Loses the renderer's device, as the platform reports a loss.
    fn lose_device(&mut self) -> bool;
}

/// Builds an unconfigured renderer whose presenter works in the given mode for a target of the
/// given size, or returns `None` when the factory builds none in that mode.
pub type MakeRig<'a> = &'a dyn Fn(PresentMode, [u32; 2]) -> Option<Box<dyn PresentRig>>;

/// Every case of the present suite, in the order [`run_all`] runs them.
pub const CASES: &[Case<fn(MakeRig<'_>)>] = &[
    case("unconfigured", unconfigured),
    case("kept_target_age", kept_target_age),
    case("undamaged_frames", undamaged_frames),
    case("held_frames", held_frames),
    case("timed_out_acquisitions", timed_out_acquisitions),
    case("device_loss", device_loss),
    case("buffer_age", buffer_age),
    case("acquire_block", acquire_block),
];

const fn case(name: &'static str, run: fn(MakeRig<'_>)) -> Case<fn(MakeRig<'_>)> {
    Case { name, run }
}

/// Runs every case of the present suite on rigs from `make`.
///
/// # Panics
///
/// Panics after the last case when any case failed, naming every failed case.
pub fn run_all(make: MakeRig<'_>) {
    crate::conformance::run_cases(CASES, |case| (case.run)(make));
}

/// Declares one test per case of the present suite, each on rigs from the factory expression
/// `$make`, which is evaluated once per test.
#[macro_export]
macro_rules! present_suite {
    ($make:expr) => {
        $crate::present_suite!(
            @cases $make;
            unconfigured, kept_target_age, undamaged_frames, held_frames,
            timed_out_acquisitions, device_loss, buffer_age, acquire_block
        );
    };
    (@cases $make:expr; $($case:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case() {
                $crate::conformance::present::$case(&$make);
            }
        )+
    };
}
