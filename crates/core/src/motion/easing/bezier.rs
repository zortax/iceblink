//! The CSS cubic Bézier easing.

/// How close the solved curve parameter's `x` comes to the input.
const PRECISION: f64 = 1e-6;

/// Returns the CSS cubic Bézier easing through `(0, 0)` and `(1, 1)` with control points
/// `[x1, y1, x2, y2]`, at progress `x`.
///
/// `x1` and `x2` are clamped to `[0, 1]`. The curve parameter is solved to within `10⁻⁶` of `x`.
/// Outside `[0, 1]` the curve continues along its tangent at the nearer end.
pub fn cubic_bezier([x1, y1, x2, y2]: [f32; 4], x: f32) -> f32 {
    let (x1, x2) = (f64::from(x1.clamp(0.0, 1.0)), f64::from(x2.clamp(0.0, 1.0)));
    let (y1, y2) = (f64::from(y1), f64::from(y2));
    let x = f64::from(x);

    if x < 0.0 {
        let slope = if x1 > 0.0 {
            y1 / x1
        } else if y1 == 0.0 && x2 > 0.0 {
            y2 / x2
        } else {
            0.0
        };
        return (slope * x) as f32;
    }
    if x > 1.0 {
        let slope = if x2 < 1.0 {
            (y2 - 1.0) / (x2 - 1.0)
        } else if y2 == 1.0 && x1 < 1.0 {
            (y1 - 1.0) / (x1 - 1.0)
        } else {
            0.0
        };
        return (1.0 + slope * (x - 1.0)) as f32;
    }

    let t = solve(x1, x2, x);
    component(y1, y2, t) as f32
}

/// Returns the curve parameter whose `x` is `x`, for `x` in `[0, 1]`.
fn solve(x1: f64, x2: f64, x: f64) -> f64 {
    // Newton's method converges in a few steps where the slope is not flat.
    let mut t = x;
    for _ in 0..8 {
        let error = component(x1, x2, t) - x;
        if error.abs() < PRECISION {
            return t;
        }
        let slope = derivative(x1, x2, t);
        if slope.abs() < 1e-9 {
            break;
        }
        t -= error / slope;
    }

    // Bisection always converges, since `x` grows monotonically with the parameter.
    let (mut low, mut high) = (0.0, 1.0);
    t = x;
    while high - low > 1e-12 {
        let value = component(x1, x2, t);
        if (value - x).abs() < PRECISION {
            return t;
        }
        if value < x {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) / 2.0;
    }
    t
}

/// Returns one coordinate of the curve at parameter `t`, from the coordinates of its control
/// points.
fn component(first: f64, second: f64, t: f64) -> f64 {
    let rest = 1.0 - t;
    3.0 * rest * rest * t * first + 3.0 * rest * t * t * second + t * t * t
}

/// Returns the derivative of `component` by `t`.
fn derivative(first: f64, second: f64, t: f64) -> f64 {
    let rest = 1.0 - t;
    3.0 * rest * rest * first + 6.0 * rest * t * (second - first) + 3.0 * t * t * (1.0 - second)
}
