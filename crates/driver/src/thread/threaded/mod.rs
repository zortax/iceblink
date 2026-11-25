//! The driver thread's owner.

mod client;

use std::panic::resume_unwind;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use iceblink::{Compositor, CompositorCapabilities};

use crate::DriverOptions;
use crate::thread::command::Command;
use crate::thread::driver;
use crate::thread::handle::ClientHandle;
use crate::thread::shared::Shared;

/// A compositor running on a driver thread of its own, with the renderers of its outputs.
///
/// The owner is a client of the thread too, through a handle of its own. Dropping it stops the
/// thread as [`shutdown`](Self::shutdown) does, and discards a panic of the driver thread.
pub struct ThreadedCompositor {
    commands: Sender<Command>,
    handle: ClientHandle,
    thread: Option<JoinHandle<()>>,
}

impl ThreadedCompositor {
    /// Moves `compositor` onto a new paced driver thread, and returns the thread's owner and a
    /// handle for clients.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn spawn(compositor: Box<dyn Compositor>) -> (Self, ClientHandle) {
        Self::spawn_with(compositor, DriverOptions::default())
    }

    /// Moves `compositor` onto a new driver thread paced as `options` say, and returns the
    /// thread's owner and a handle for clients.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn spawn_with(
        compositor: Box<dyn Compositor>,
        options: DriverOptions,
    ) -> (Self, ClientHandle) {
        let capabilities = CompositorCapabilities {
            threaded: true,
            ..compositor.capabilities()
        };
        let fonts = compositor.font_registry();
        let shared = Arc::new(Shared::new(capabilities, fonts, options.pacing));
        let (commands, received) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("compositor".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || driver::run(compositor, options, received, shared)
            })
            .expect("the compositor thread starts");
        shared.set_driver(thread.thread().id());

        let handle = ClientHandle::new(commands.clone(), shared);
        let owner = Self {
            commands,
            handle: handle.clone(),
            thread: Some(thread),
        };
        (owner, handle)
    }

    /// Stops the thread after the frame in progress.
    ///
    /// Calls queued before the shutdown are applied first. The compositor and every renderer are
    /// then dropped on the driver thread.
    ///
    /// # Panics
    ///
    /// Panics with the driver thread's panic if it panicked.
    pub fn shutdown(mut self) {
        if let Err(panic) = self.stop() {
            resume_unwind(panic);
        }
    }

    fn stop(&mut self) -> thread::Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        let _ = self.commands.send(Command::Shutdown);
        thread.join()
    }
}

impl Drop for ThreadedCompositor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
