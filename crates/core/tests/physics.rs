//! Fling decay, the overscroll bounce and the step curve.

use iceblink_core::motion::physics::{arrived, bounce, fling, rest_speed, step};
use iceblink_core::motion::spring::Spring;

#[test]
fn a_fling_decays_by_the_spec_formula() {
    let r: f64 = 0.002;
    let (v, s) = (3000.0, 0.25);
    assert!((fling::distance(v, s) - v * (1.0 - r.powf(s)) / -r.ln()).abs() < 1e-9);
    assert!((fling::velocity(v, s) - v * r.powf(s)).abs() < 1e-9);
    let stop = fling::stop_time(v, 144.0);
    assert!((fling::velocity(v, stop) - 144.0).abs() < 1e-9);
    assert_eq!(fling::stop_time(100.0, 144.0), 0.0);
}

#[test]
fn the_rest_speed_is_a_device_pixel_per_refresh() {
    assert!((rest_speed(6_944_444) - 144.0).abs() < 1e-3);
    assert!((rest_speed(0) - 60.0).abs() < 1e-3);
}

#[test]
fn a_pull_displaces_by_a_softened_distance_and_the_pull_inverts_it() {
    let extent = 400.0;
    let shown = bounce::displacement(100.0, extent);
    assert!((shown - 400.0 * (1.0 - 1.0 / (0.55 * 100.0 / 400.0 + 1.0))).abs() < 1e-9);
    assert!((bounce::pull(shown, extent) - 100.0).abs() < 1e-9);
    assert!(bounce::displacement(-100.0, extent) < 0.0);
    // The extent counts as at least one pixel.
    assert_eq!(
        bounce::displacement(10.0, 0.0),
        bounce::displacement(10.0, 1.0)
    );
}

#[test]
fn a_return_decays_to_rest() {
    let (d, v) = bounce::returning(30.0, -200.0, 0.1);
    let expected = (30.0 + 0.31 * -200.0 * 0.1) * (-1.25_f64).exp();
    assert!((d - expected).abs() < 1e-9);
    assert!(v < 0.0);
    let (d, v) = bounce::returning(30.0, 0.0, 1.0);
    assert!(arrived(d, v, rest_speed(0)));
}

#[test]
fn a_step_curve_eases_out_over_240_ms() {
    assert_eq!(step::share(0), 0.0);
    assert_eq!(step::share(step::DURATION_NS), 1.0);
    for eighths in 1..8 {
        let p = eighths as f32 / 8.0;
        let share = step::share(step::DURATION_NS * eighths / 8);
        assert!(
            (share - (1.0 - (1.0 - p).powi(3))).abs() < 1e-6,
            "{share} at {p}"
        );
    }
}

#[test]
fn the_snap_spring_starts_where_it_is_told() {
    let (y, v) = Spring::SNAP.at(100.0, -50.0, 0.0);
    assert!((y - 100.0).abs() < 1e-9 && (v + 50.0).abs() < 1e-9);
    let (y, v) = Spring::SNAP.at(100.0, 0.0, 2.0);
    assert!(y.abs() < 0.5 && v.abs() < 1.0);
}
