//! Compile-time checks of the sizes and alignments the contract fixes for the Pod frame kinds.

use crate::frame::{
    AtlasTile, ColorSprite, FrameMarks, FrameShadow, FrameStop, ItemSource, MarkCell, Sprite,
};
use crate::layout::layout;

layout! {
    AtlasTile => 20, 4;
    Sprite => 64, 4;
    ColorSprite => 132, 4;
    FrameShadow => 160, 4;
    FrameMarks => 100, 4;
    MarkCell => 24, 4;
    ItemSource => 12, 4;
    FrameStop => 20, 4;
}
