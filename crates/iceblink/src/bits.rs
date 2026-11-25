//! Macros that declare the `u8` and `u32` newtypes used as enumerations and bit sets in Pod values.

/// Declares an enumeration as a newtype with one associated constant per value.
macro_rules! enumeration {
    (
        $(#[$meta:meta])*
        pub struct $name:ident($repr:ty) {
            $( $(#[$value_meta:meta])* $value:ident = $bits:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable,
        )]
        pub struct $name(pub $repr);

        impl $name {
            $(
                $(#[$value_meta])*
                pub const $value: Self = Self($bits);
            )+

            /// Reports whether the value is one of the listed constants.
            pub const fn is_valid(self) -> bool {
                $(self.0 == $bits)||+
            }
        }
    };
}

/// Declares a bit set as a newtype with one associated constant per bit.
macro_rules! bit_set {
    (
        $(#[$meta:meta])*
        pub struct $name:ident($repr:ty) {
            $( $(#[$bit_meta:meta])* $bit:ident = $bits:expr, )+
        }
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable,
        )]
        pub struct $name(pub $repr);

        impl $name {
            $(
                $(#[$bit_meta])*
                pub const $bit: Self = Self($bits);
            )+

            /// Reports whether every bit of `other` is set.
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            /// Reports whether no bit outside the listed constants is set.
            pub const fn is_valid(self) -> bool {
                self.0 & !(0 $(| $bits)+) == 0
            }
        }

        impl core::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
        }

        impl core::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, other: Self) {
                self.0 |= other.0;
            }
        }
    };
}

pub(crate) use {bit_set, enumeration};
