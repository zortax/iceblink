//! The window, its event loop, and the threaded compositor that draws into it.

use std::sync::{Arc, Mutex};

use iceblink::OutputId;
use iceblink::{Compositor as _, CompositorClient, OutputConfig, RendererFactory};
use iceblink_core::CompositorImpl;
use iceblink_driver::thread::{ClientHandle, ThreadedCompositor};
use iceblink_examples::{Font, logs_frames, refresh_ns};
use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_text::SwashRaster;
use iceblink_wgpu::{Builder, SurfacePresenter};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use crate::gesture::Gestures;
use crate::list::List;
use crate::logged::Logged;

/// The user event the client's waker sends: events wait for the client.
struct Wake;

/// Opens the window and runs until it is closed.
///
/// # Panics
///
/// Panics if no event loop, window or graphics device can be made.
pub(crate) fn run() {
    let event_loop = EventLoop::<Wake>::with_user_event()
        .build()
        .expect("an event loop opens");
    let mut app = App {
        proxy: event_loop.create_proxy(),
        shown: None,
    };
    event_loop.run_app(&mut app).expect("the event loop runs");
}

struct App {
    proxy: EventLoopProxy<Wake>,
    shown: Option<Shown>,
}

/// A window, the compositor that draws it, and the content it shows.
struct Shown {
    window: Arc<Window>,
    driver: ThreadedCompositor,
    handle: ClientHandle,
    output: OutputId,
    list: List,
    gestures: Gestures,
    /// The pointer's position, in device pixels.
    cursor: [f32; 2],
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("iceblink threaded")
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("a window opens"),
        );
        self.shown = Some(Shown::open(window, self.proxy.clone()));
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: Wake) {
        if let Some(shown) = self.shown.as_mut() {
            shown.list.observe(&mut shown.handle);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => shown.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => shown.resize(shown.window.inner_size()),
            WindowEvent::CursorMoved { position, .. } => {
                shown.cursor = [position.x as f32, position.y as f32];
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let scale = shown.window.scale_factor() as f32;
                let gesture = shown.gestures.step(delta, phase, shown.cursor, scale);
                // A gesture no node takes would scroll an outer window, and this one has none.
                let predicted = shown.list.predict(shown.cursor, scale);
                let client = shown.list.client();
                shown
                    .handle
                    .scroll_input(client, shown.output, gesture, predicted);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                shown.list.toggle_marker(&mut shown.handle);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let hits = shown.handle.hit_test(shown.output, shown.cursor);
                match List::row(&hits) {
                    Some(row) => println!("hit row={row}"),
                    None => println!("hit nothing"),
                }
            }
            _ => {}
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(shown) = self.shown.take() {
            shown.driver.shutdown();
        }
    }
}

impl Shown {
    fn open(window: Arc<Window>, proxy: EventLoopProxy<Wake>) -> Self {
        let mut compositor = CompositorImpl::new();
        compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
        let data = read_font(LATIN);
        let font = Font {
            id: compositor.font_registry().register(data.clone()),
            data,
        };

        let (driver, mut handle) = ThreadedCompositor::spawn(Box::new(compositor));
        let (client, _) = handle.register_client();
        let proxy = Mutex::new(proxy);
        let waker = Arc::new(move || {
            let _ = proxy.lock().expect("the proxy is whole").send_event(Wake);
        });
        handle.set_waker(client, Some(waker));
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let output = handle.create_output(config(size, scale), renderer(Arc::clone(&window)));
        handle.set_refresh(output, refresh_ns(&window));
        let list = List::open(
            &mut handle,
            client,
            output,
            font,
            logical(size, scale),
            scale,
        );
        Self {
            window,
            driver,
            handle,
            output,
            list,
            gestures: Gestures::default(),
            cursor: [0.0; 2],
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        let scale = self.window.scale_factor() as f32;
        self.handle
            .set_refresh(self.output, refresh_ns(&self.window));
        self.handle
            .configure_output(self.output, config(size, scale));
        self.list
            .resize(&mut self.handle, logical(size, scale), scale);
    }
}

/// Returns a factory of renderers that present into `window`, which fails when no device presents
/// to the window.
///
/// The renderers print a line for every frame if [`logs_frames`] holds.
fn renderer(window: Arc<Window>) -> RendererFactory {
    Box::new(move || {
        let size = window.inner_size();
        let mut renderer = Builder::new().for_surface([size.width, size.height], true, window)?;
        let adapter = renderer.gpu().describe();
        let timed = renderer.wgpu_presenter().capabilities().feedback;
        let native = renderer
            .wgpu_presenter()
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<SurfacePresenter>())
            .map(|surface| surface.is_native());
        println!("opened adapter={adapter} native={native:?} timed={timed}");
        if logs_frames() {
            Ok(Box::new(Logged { inner: renderer }))
        } else {
            Ok(Box::new(renderer))
        }
    })
}

fn config(size: PhysicalSize<u32>, scale: f32) -> OutputConfig {
    OutputConfig {
        size: [size.width.max(1), size.height.max(1)],
        scale,
        opaque: true,
    }
}

fn logical(size: PhysicalSize<u32>, scale: f32) -> [f32; 2] {
    [size.width as f32 / scale, size.height as f32 / scale]
}
