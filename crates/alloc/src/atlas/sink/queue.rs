//! Texture creations and destructions, queued for a device.

use iceblink::geom::{Device, Size};

use crate::atlas::sink::TextureSink;
use crate::atlas::sink::error::SinkError;
use crate::atlas::texture::{TextureFormat, TextureId};

/// A queued texture operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextureOp {
    Create {
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
        mip_levels: u32,
    },
    Destroy {
        texture: TextureId,
    },
}

/// The texture creations and destructions not yet performed, in the order they were queued.
// One ordered log, because a slot destroyed and then created again must replay in that order.
#[derive(Debug, Default)]
pub(crate) struct TextureQueue {
    ops: Vec<TextureOp>,
}

impl TextureQueue {
    /// Creates an empty queue.
    pub(crate) fn new() -> Self {
        Self { ops: Vec::new() }
    }

    /// Queues the creation of `texture` with one level.
    pub(crate) fn create(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
    ) {
        self.create_with_mips(texture, size, format, 1);
    }

    /// Queues the creation of `texture` with `mip_levels` levels of detail.
    pub(crate) fn create_with_mips(
        &mut self,
        texture: TextureId,
        size: Size<i32, Device>,
        format: TextureFormat,
        mip_levels: u32,
    ) {
        self.ops.push(TextureOp::Create {
            texture,
            size,
            format,
            mip_levels,
        });
    }

    /// Queues the destruction of `texture`.
    ///
    /// If the creation of `texture` is still queued, it is cancelled and nothing is queued.
    pub(crate) fn destroy(&mut self, texture: TextureId) {
        let last = self.ops.iter().rposition(|op| match op {
            TextureOp::Create { texture: made, .. } => *made == texture,
            TextureOp::Destroy { texture: gone } => *gone == texture,
        });
        if let Some(index) = last
            && matches!(self.ops[index], TextureOp::Create { .. })
        {
            self.ops.remove(index);
            return;
        }
        self.ops.push(TextureOp::Destroy { texture });
    }

    /// Returns the number of queued operations.
    pub(crate) fn len(&self) -> usize {
        self.ops.len()
    }

    /// Performs the queued operations against `sink`, oldest first.
    ///
    /// # Errors
    ///
    /// Returns the [`SinkError`] of a refused creation. The refused operation and everything after
    /// it stay queued.
    pub(crate) fn replay(&mut self, sink: &mut impl TextureSink) -> Result<(), SinkError> {
        let mut done = 0;
        let mut failure = None;
        for op in &self.ops {
            match *op {
                TextureOp::Create {
                    texture,
                    size,
                    format,
                    mip_levels,
                } => {
                    if let Err(error) =
                        sink.create_texture_with_mips(texture, size, format, mip_levels)
                    {
                        failure = Some(error);
                        break;
                    }
                }
                TextureOp::Destroy { texture } => sink.destroy_texture(texture),
            }
            done += 1;
        }
        self.ops.drain(..done);
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
