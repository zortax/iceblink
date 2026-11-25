//! A damped spring in closed form.

/// A mass on a damped spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// The mass.
    pub mass: f32,
    /// The stiffness of the spring.
    pub stiffness: f32,
    /// The damping coefficient.
    pub damping: f32,
}

impl Spring {
    /// The spring of snap motion.
    pub const SNAP: Self = Self {
        mass: 1.0,
        stiffness: 170.0,
        damping: 26.0,
    };

    /// Returns the displacement from rest and the velocity `t` seconds after the mass starts at
    /// displacement `y0` with velocity `v0`.
    ///
    /// The damping ratio `ζ = damping / (2 √(stiffness × mass))` selects the underdamped,
    /// critically damped or overdamped solution.
    pub fn at(&self, y0: f64, v0: f64, t: f64) -> (f64, f64) {
        let (mass, stiffness, damping) = (
            f64::from(self.mass),
            f64::from(self.stiffness),
            f64::from(self.damping),
        );
        let omega = (stiffness / mass).sqrt();
        let zeta = damping / (2.0 * (stiffness * mass).sqrt());

        if zeta < 1.0 {
            let damped = omega * (1.0 - zeta * zeta).sqrt();
            let decay = (-zeta * omega * t).exp();
            let (sin, cos) = (damped * t).sin_cos();
            let b = (v0 + zeta * omega * y0) / damped;
            let y = decay * (y0 * cos + b * sin);
            let v = decay
                * ((b * damped - zeta * omega * y0) * cos - (y0 * damped + zeta * omega * b) * sin);
            (y, v)
        } else if zeta == 1.0 {
            let decay = (-omega * t).exp();
            let b = v0 + omega * y0;
            let y = decay * (y0 + b * t);
            let v = decay * (b - omega * (y0 + b * t));
            (y, v)
        } else {
            let root = (zeta * zeta - 1.0).sqrt();
            let r1 = -omega * (zeta - root);
            let r2 = -omega * (zeta + root);
            let a = (v0 - r2 * y0) / (r1 - r2);
            let b = y0 - a;
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            (a * e1 + b * e2, a * r1 * e1 + b * r2 * e2)
        }
    }
}
