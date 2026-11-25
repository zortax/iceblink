//! The two-dimensional transforms a 4x4 matrix can embed.

use crate::geom::transform::{Affine2, Matrix4};

impl Matrix4 {
    /// Reports whether the matrix maps the z = 0 plane onto itself as an [`Affine2`] would.
    pub fn is_2d(&self) -> bool {
        let m = &self.columns;
        m[0][2] == 0.0
            && m[0][3] == 0.0
            && m[1][2] == 0.0
            && m[1][3] == 0.0
            && m[2] == [0.0, 0.0, 1.0, 0.0]
            && m[3][2] == 0.0
            && m[3][3] == 1.0
    }

    /// Returns the offset the matrix moves points of the z = 0 plane by, or `None` if it does more
    /// than translate them.
    ///
    /// ```
    /// use iceblink::geom::Matrix4;
    ///
    /// assert_eq!(Matrix4::translation(3.0, -2.5, 0.0).plane_translation(), Some((3.0, -2.5)));
    /// assert_eq!(Matrix4::scale(2.0, 1.0, 1.0).plane_translation(), None);
    /// ```
    pub fn plane_translation(&self) -> Option<(f32, f32)> {
        // The third column multiplies z, which is zero in the plane.
        let [x, y, _, w] = self.columns;
        let moves_only = x[0] == 1.0
            && x[1] == 0.0
            && x[3] == 0.0
            && y[0] == 0.0
            && y[1] == 1.0
            && y[3] == 0.0
            && w[3] == 1.0;
        moves_only.then_some((w[0], w[1]))
    }

    /// Returns the two-dimensional transform this matrix embeds, or `None` if it is not
    /// [two-dimensional](Matrix4::is_2d).
    ///
    /// ```
    /// use iceblink::geom::{Affine2, Matrix4};
    ///
    /// let turned = Affine2::rotation(0.4);
    /// assert_eq!(turned.to_matrix4().to_affine2(), Some(turned));
    /// assert_eq!(Matrix4::perspective(400.0).to_affine2(), None);
    /// ```
    pub fn to_affine2(&self) -> Option<Affine2> {
        let m = &self.columns;
        self.is_2d()
            .then(|| Affine2::new(m[0][0], m[0][1], m[1][0], m[1][1], m[3][0], m[3][1]))
    }
}
