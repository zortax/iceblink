//! A browser canvas as a surface target.

/// A browser canvas to draw into.
#[derive(Clone, Debug)]
pub struct Canvas(pub wgpu::web_sys::HtmlCanvasElement);

impl From<Canvas> for wgpu::SurfaceTarget<'static> {
    fn from(canvas: Canvas) -> Self {
        Self::Canvas(canvas.0)
    }
}
