//! The arithmetic operators of the length newtypes.

/// Implements the arithmetic operators for a single-field length newtype.
macro_rules! length_ops {
    ($name:ident, $inner:ty) => {
        impl ::core::ops::Add for $name {
            type Output = Self;

            #[inline]
            fn add(self, other: Self) -> Self {
                Self(self.0 + other.0)
            }
        }

        impl ::core::ops::AddAssign for $name {
            #[inline]
            fn add_assign(&mut self, other: Self) {
                self.0 += other.0;
            }
        }

        impl ::core::ops::Sub for $name {
            type Output = Self;

            #[inline]
            fn sub(self, other: Self) -> Self {
                Self(self.0 - other.0)
            }
        }

        impl ::core::ops::SubAssign for $name {
            #[inline]
            fn sub_assign(&mut self, other: Self) {
                self.0 -= other.0;
            }
        }

        impl ::core::ops::Neg for $name {
            type Output = Self;

            #[inline]
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl ::core::ops::Mul<f32> for $name {
            type Output = Self;

            #[inline]
            fn mul(self, factor: f32) -> Self {
                Self::from_scaled(self.0, factor)
            }
        }

        impl ::core::ops::MulAssign<f32> for $name {
            #[inline]
            fn mul_assign(&mut self, factor: f32) {
                *self = *self * factor;
            }
        }

        impl ::core::ops::Div<f32> for $name {
            type Output = Self;

            #[inline]
            fn div(self, divisor: f32) -> Self {
                Self::from_divided(self.0, divisor)
            }
        }

        impl ::core::ops::Div for $name {
            type Output = f32;

            #[inline]
            fn div(self, divisor: Self) -> f32 {
                $crate::geom::unit::Unit::to_f32(self) / $crate::geom::unit::Unit::to_f32(divisor)
            }
        }

        impl ::core::iter::Sum for $name {
            #[inline]
            fn sum<I: Iterator<Item = Self>>(iterator: I) -> Self {
                iterator.fold(Self(<$inner as Default>::default()), ::core::ops::Add::add)
            }
        }
    };
}

pub(crate) use length_ops;
