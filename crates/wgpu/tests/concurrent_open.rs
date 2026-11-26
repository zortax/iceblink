//! Renderers opened and dropped from several threads at once.
//!
//! The first instances of a process are created here, so this test keeps a binary of its own.

use std::sync::{Arc, Barrier};

use iceblink_wgpu::{Builder, wgpu};

#[test]
fn renderers_open_and_drop_on_several_threads_at_once() {
    const THREADS: usize = 4;
    const ROUNDS: usize = 4;
    let start = Arc::new(Barrier::new(THREADS));
    let threads: Vec<_> = (0..THREADS)
        .map(|_| {
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                (0..ROUNDS).try_for_each(|_| {
                    Builder::new()
                        .offscreen([16, 16], wgpu::TextureFormat::Bgra8Unorm)
                        .map(drop)
                        .map_err(|failure| failure.to_string())
                })
            })
        })
        .collect();

    for thread in threads {
        let opened = thread.join().expect("opening a renderer does not panic");
        if let Err(failure) = opened {
            panic!("no usable graphics device: {failure}");
        }
    }
}
