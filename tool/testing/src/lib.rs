//! Shared fakes, fixtures, drivers and conformance checks for the iceblink crates.
//!
//! | Module | Contents |
//! | --- | --- |
//! | [`alloc`] | [`CountingAlloc`], a global allocator that counts one thread's allocations |
//! | [`conformance`] | The scene, tier, draw, present and end-to-end suites, which any implementation runs through a factory |
//! | [`validate`] | [`validate_frame`] and [`FrameValidator`], which check the frame contract on any frame |
//! | [`golden`] | Golden text and PNG files, compared within a tolerance and blessed with `ICEBLINK_BLESS=1` |
//! | [`gpu`] | A device on hardware or lavapipe, the device lock, and texture readback to RGBA |
//! | [`fakes`] | A glyph rasteriser, a font registry and a renderer that need no fonts and no device |
//! | [`fonts`] | The font files the tests read |
//! | [`vector`] | Hand-built vector frames drawn through a path rasteriser and composited on the host |
//! | [`capture`] | A recording renderer and stable frame transcripts |
//! | [`naive`] | A second, naive compositor that implements the shared compositor contract |

pub mod alloc;
pub mod capture;
pub mod conformance;
pub mod fakes;
pub mod fonts;
pub mod golden;
pub mod gpu;
pub mod naive;
pub mod validate;
pub mod vector;

pub use crate::alloc::{CountingAlloc, allocations};
pub use crate::validate::{
    FrameValidator, FrameViolation, Rule, assert_valid_frame, validate_frame,
};
