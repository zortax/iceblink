//! The easing functions, checked against their closed-form formulas.

use std::sync::Arc;

use iceblink::{Easing, StepJump};
use iceblink_core::motion::easing::{cubic_bezier, ease, linear_points, steps};

fn close(actual: f32, expected: f32, within: f32) {
    assert!(
        (actual - expected).abs() <= within,
        "{actual} is not within {within} of {expected}"
    );
}

#[test]
fn a_cubic_bezier_solves_the_css_curve() {
    // CSS `ease` at one half, from the curve solved in double precision.
    close(cubic_bezier([0.25, 0.1, 0.25, 1.0], 0.5), 0.802_403_4, 1e-5);
    close(cubic_bezier([0.42, 0.0, 0.58, 1.0], 0.5), 0.5, 1e-6);
    assert_eq!(cubic_bezier([0.25, 0.1, 0.25, 1.0], 0.0), 0.0);
    close(cubic_bezier([0.25, 0.1, 0.25, 1.0], 1.0), 1.0, 1e-6);
}

#[test]
fn a_cubic_bezier_clamps_its_horizontal_control_points() {
    for x in [0.1, 0.5, 0.9] {
        close(
            cubic_bezier([1.5, 0.3, -0.5, 0.7], x),
            cubic_bezier([1.0, 0.3, 0.0, 0.7], x),
            1e-6,
        );
    }
}

#[test]
fn a_cubic_bezier_continues_along_its_end_tangents() {
    // The tangent at the start is y1 / x1 = 2, and at the end (1 - y2) / (1 - x2) = 0.5.
    close(cubic_bezier([0.25, 0.5, 0.5, 0.75], -0.5), -1.0, 1e-6);
    close(cubic_bezier([0.25, 0.5, 0.5, 0.75], 1.5), 1.25, 1e-6);
}

#[test]
fn steps_jump_where_their_position_says() {
    close(steps(4, StepJump::End, 0.3), 0.25, 0.0);
    close(steps(4, StepJump::Start, 0.3), 0.5, 0.0);
    close(steps(5, StepJump::None, 0.5), 0.5, 0.0);
    close(steps(3, StepJump::Both, 0.0), 0.25, 0.0);
    close(steps(3, StepJump::Both, 1.0), 1.0, 0.0);
    close(steps(4, StepJump::End, 1.0), 1.0, 0.0);
    close(steps(4, StepJump::Start, 0.0), 0.25, 0.0);
}

#[test]
fn steps_take_any_count() {
    close(steps(u32::MAX, StepJump::Both, 0.5), 0.5, 1e-6);
    close(steps(u32::MAX, StepJump::None, 1.0), 1.0, 0.0);
}

#[test]
fn linear_points_hold_flat_outside_their_inputs() {
    let points = [[0.2, 0.1], [0.5, 0.8], [0.5, 0.6], [1.0, 1.0]];
    close(linear_points(&points, 0.0), 0.1, 0.0);
    close(linear_points(&points, 0.35), 0.45, 1e-6);
    // The later of two points at one input wins from that input on.
    close(linear_points(&points, 0.5), 0.6, 1e-6);
    close(linear_points(&points, 0.75), 0.8, 1e-6);
    close(linear_points(&points, 1.5), 1.0, 0.0);
    let eased = ease(&Easing::LinearPoints(Arc::from(points)), 0.35, 1.0);
    close(eased, 0.45, 1e-6);
}

/// Returns the spring output `1 - e(τ)` in closed form, in the three damping regimes.
fn closed_form_spring(mass: f64, stiffness: f64, damping: f64, velocity: f64, tau: f64) -> f64 {
    let omega = (stiffness / mass).sqrt();
    let zeta = damping / (2.0 * (stiffness * mass).sqrt());
    let e = if zeta < 1.0 {
        let damped = omega * (1.0 - zeta * zeta).sqrt();
        (-zeta * omega * tau).exp()
            * ((damped * tau).cos() + ((zeta * omega - velocity) / damped) * (damped * tau).sin())
    } else if zeta == 1.0 {
        (-omega * tau).exp() * (1.0 + (omega - velocity) * tau)
    } else {
        let root = (zeta * zeta - 1.0).sqrt();
        let (r1, r2) = (-omega * (zeta - root), -omega * (zeta + root));
        let a = (-velocity - r2) / (r1 - r2);
        a * (r1 * tau).exp() + (1.0 - a) * (r2 * tau).exp()
    };
    1.0 - e
}

#[test]
fn a_spring_follows_the_damped_mass_in_every_regime() {
    // Underdamped, critically damped and overdamped, each with a starting velocity.
    for (mass, stiffness, damping, velocity) in [
        (1.0, 100.0, 10.0, 2.0),
        (1.0, 100.0, 20.0, -3.0),
        (2.0, 50.0, 40.0, 1.0),
    ] {
        let easing = Easing::Spring {
            mass,
            stiffness,
            damping,
            velocity,
        };
        for x in [0.05, 0.2, 0.5, 0.9] {
            let expected = closed_form_spring(
                f64::from(mass),
                f64::from(stiffness),
                f64::from(damping),
                f64::from(velocity),
                f64::from(x) * 0.8,
            );
            close(ease(&easing, x, 0.8), expected as f32, 1e-5);
        }
        assert_eq!(ease(&easing, 1.0, 0.8), 1.0);
    }
}
