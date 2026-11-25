//! The interface of iceblink: the value types and traits of the shared compositor contract.
//!
//! Clients describe what to draw with the value types: keyed chunks of Pod items under transform,
//! scroll, clip and effect nodes, an order tree that sets paint order, paint and parameter slots,
//! and resources. A [`Transaction`] collects changes to all of them, and a compositor applies it
//! atomically. The compositor answers with a [`Frame`], the finished plan a renderer executes, and
//! renderers and presenters report what they support and what drawing did.
//!
//! The contract has three seams, each a trait here. A client talks to a [`Compositor`], which
//! turns transactions into frames. A [`Renderer`] draws each frame, and a [`Presenter`] puts the
//! drawn images on the display. A client that leaves the frame lifecycle to a driver talks to it
//! through a [`CompositorClient`]. A compositor rasterises glyphs through a [`GlyphRaster`] and
//! knows fonts through a [`FontRegistry`]. Every trait is object-safe.
//!
//! The value types are built from the foundation modules [`geom`], [`color`] and [`damage`], and
//! from the re-exported [`kurbo`] and [`smallvec`] crates, so a client builds every value through
//! this crate alone.

mod bits;
mod client;
mod clock;
mod compositor;
mod glyph;
mod layout;
mod premultiplied;
mod presenter;
mod renderer;

pub use kurbo;
pub use smallvec;

pub mod animation;
pub mod chunk;
pub mod color;
pub mod commit;
pub mod counters;
pub mod damage;
pub mod event;
pub mod font;
pub mod frame;
pub mod geom;
pub mod geometry;
pub mod id;
pub mod input;
pub mod item;
pub mod node;
pub mod paint;
pub mod render;
pub mod resource;
pub mod shader;
pub mod text;
pub mod transaction;

pub use crate::animation::{
    Animation, AnimationStart, AnimationTarget, Direction, Easing, FillMode, FilterParam, Keyframe,
    Keyframes, StepJump, Timing,
};
pub use crate::chunk::{ChunkFlags, ChunkKey, ChunkProps, LocalClip, LocalParent};
pub use crate::client::{
    ClientEvent, CommitId, CompositorClient, DriverPacing, DriverStats, EventWaker, FrameObserver,
    OffsetRange, OutputStatus, OutputWatch, RendererFactory, RendererTask, ScrollWatch, WatchCause,
};
pub use crate::clock::now_ns;
pub use crate::commit::{CommitError, RejectReason};
pub use crate::compositor::{
    CommitReport, Compositor, CompositorBudgets, CompositorCapabilities, CompositorMemory,
    OutputConfig,
};
pub use crate::counters::{CommitCounters, Counters, FrameCounters};
pub use crate::event::{AnimationEvent, CompositorEvent, ScrollPhase};
pub use crate::font::{FontBytes, FontData, FontId, FontRegistry};
pub use crate::frame::{
    AtlasTile, Batch, BatchKind, Built, ChunkNote, ClipEntry, ClipLinkEntry, ColorSprite,
    DrawnChunk, FRAME_NO_CLIP, Frame, FrameBackdrop, FrameClipShape, FrameClock, FrameDamage,
    FrameExternal, FrameFilter, FrameGradient, FrameGroup, FrameMarkPayload, FrameMarks,
    FramePaint, FramePath, FrameRequest, FrameShadow, FrameStop, Instance, ItemSource, Lane, Lanes,
    LoweredChunk, MAX_MARK_PAGES, MarkBin, MarkCell, MarkPlan, OpaqueCover, PageFormat, PageOp,
    PassKey, PlannedPass, PlannedPath, ResourceWork, ShaderClock, ShaderOp, Shift, Sprite,
    SpriteFlags, StopEncoding, TableDelta, Tables, VectorPlan,
};
pub use crate::geometry::{DeviceRect, LocalRect, Matrix4};
pub use crate::glyph::GlyphRaster;
pub use crate::id::{
    AnimationId, AnyId, ClientId, ClipId, EffectId, FrameId, Id, IdKind, IdSpace, ImageId, OrderId,
    OutputId, PaintId, ParamsId, ScrollId, ShaderId, TextureId, TransformId,
};
pub use crate::input::{GesturePhase, HitResult, ScrollGesture, ScrollRouting, ScrollSource};
pub use crate::item::{
    ArenaRange, Backdrop, BorderStyle, ClipRef, CornerShapes, Extend, ExternalQuad, FillRule,
    Glyph, GlyphFlags, GlyphRun, Image, ImageFilter, ImageFlags, ItemKind, ItemOp, Line, LineStyle,
    MarkBox, MarkFlags, MarkPayload, MarkPlacement, Marks, PathClip, PathFill, PathItem,
    PathStroke, Quad, ROUND, ShaderQuad, Shadow, SpaceRef,
};
pub use crate::node::{
    Anchor, BlendMode, ClipNode, ClipShape, EffectNode, Filter, MaskMode, OrderEntry, OrderNode,
    ScrollBehaviour, ScrollNode, ScrollPhysics, SnapKind, SnapPoint, Sources, StickyAnchor,
    ThumbAnchor, TransformNode, read_extent,
};
pub use crate::paint::{
    ColorSpace, Gradient, GradientShape, GradientStop, HueInterpolation, Interpolation, Paint,
    PaintKind, PaintRef, ParamsBlock,
};
pub use crate::premultiplied::Color;
pub use crate::presenter::Presenter;
pub use crate::render::vector::{VectorBackend, VectorStatus};
pub use crate::render::{
    Acquired, BufferAge, FrameOutcome, FrameStats, GpuUnavailable, MemoryReport,
    PresentCapabilities, PresentFeedback, PresentMode, PresentResult, RejectedAdapter,
    RenderCapabilities, RenderTarget, STALE_DAMAGE, SkipReason, SubpixelOrder, TargetPoolReport,
};
pub use crate::renderer::Renderer;
pub use crate::resource::{
    ImageDesc, ImageFormat, ImageWrite, Prebuilt, ShaderDesc, ShaderMode, ShaderReads,
    ShaderSource, TextureDesc,
};
pub use crate::text::{
    GlyphBitmap, GlyphKey, GlyphPlacement, OutlineKey, PHASES, PenPosition, RasterStyle,
};
pub use crate::transaction::{
    ChunkRecorder, Command, RecordedChunk, Transaction, TransactionParts,
};
