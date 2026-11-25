//! Easing functions, which map progress in `[0, 1]` to eased progress.

mod bezier;
mod points;
mod steps;

use iceblink::Easing;

use crate::motion::spring::Spring;

pub use self::bezier::cubic_bezier;
pub use self::points::linear_points;
pub use self::steps::steps;

/// Returns the eased progress of `easing` at progress `x`.
///
/// Only a spring reads `duration_s`, the length of one iteration in seconds. A spring is
/// evaluated `x × duration_s` seconds after its start, and is exactly 1 at `x = 1`.
pub fn ease(easing: &Easing, x: f32, duration_s: f32) -> f32 {
    match easing {
        Easing::Linear => x,
        Easing::CubicBezier(points) => cubic_bezier(*points, x),
        Easing::Steps { count, jump } => steps(*count, *jump, x),
        Easing::LinearPoints(points) => linear_points(points, x),
        Easing::Spring {
            mass,
            stiffness,
            damping,
            velocity,
        } => {
            if x == 1.0 {
                return 1.0;
            }

            let spring = Spring {
                mass: *mass,
                stiffness: *stiffness,
                damping: *damping,
            };
            let (remaining, _) = spring.at(1.0, -f64::from(*velocity), f64::from(x * duration_s));
            (1.0 - remaining) as f32
        }
    }
}
