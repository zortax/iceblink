//! The CSS step easing.

use iceblink::StepJump;

/// Returns the CSS step easing with `count` steps and jumps at `jump`, at progress `x`.
///
/// A count of zero, or of one with `StepJump::None`, is treated as the smallest count the jump
/// allows.
pub fn steps(count: u32, jump: StepJump, x: f32) -> f32 {
    let count = f64::from(match jump {
        StepJump::None => count.max(2),
        StepJump::Start | StepJump::End | StepJump::Both => count.max(1),
    });
    let jumps = match jump {
        StepJump::Start | StepJump::End => count,
        StepJump::None => count - 1.0,
        StepJump::Both => count + 1.0,
    };

    let mut step = (f64::from(x) * count).floor();
    if matches!(jump, StepJump::Start | StepJump::Both) {
        step += 1.0;
    }
    if x >= 0.0 && step < 0.0 {
        step = 0.0;
    }
    if x <= 1.0 && step > jumps {
        step = jumps;
    }
    (step / jumps) as f32
}
