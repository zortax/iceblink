//! Shader parameter slots.

use bytemuck::{Pod, Zeroable};

/// The parameter block of a shader parameter slot, which the client writes in full.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ParamsBlock {
    /// The pointer, in the item's local units, from its bounds origin.
    pub pointer: [f32; 2],
    /// One while the pointer is over the item, zero otherwise.
    pub hovered: f32,
    /// Zero.
    pub _reserved: f32,
    /// The shader's own parameters, zero-padded.
    pub user: [u8; 64],
}
