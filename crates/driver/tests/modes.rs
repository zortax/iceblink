//! The driver runs the frame lifecycle of kept-target and buffer-age presenters, so either
//! presents what the compositor shows.

mod common;

use std::sync::Arc;

use iceblink::{CompositorClient, RendererFactory};
use iceblink_cpu::{CpuRenderer, Pixmap, PixmapPresenter};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, ROW, commit, spawn};
use crate::common::thread::ticks::Ticks;

/// Returns a factory of CPU renderers presenting through `presenter` at the fake display's rate.
fn cpu(probe: &Arc<Probe>, presenter: fn() -> PixmapPresenter) -> RendererFactory {
    let probe = Arc::clone(probe);
    Box::new(move || {
        let ticks = Ticks::new(presenter(), PERIOD_NS, probe);
        Ok(Box::new(CpuRenderer::new(Box::new(ticks))))
    })
}

/// Reports whether the middle column of `image` shows the list scrolled to `offset`: the pixels
/// of rows of one parity are equal, and differ from the other parity's.
fn shows(image: &Pixmap, offset: f32) -> bool {
    let row = |y: u32| ((y as f32 + 0.5 + offset) / ROW) as u32;
    let pixel = |y: u32| image.pixel(100, y).expect("the pixel is inside");
    let parity = |even: bool| (0..200).find(|&y| (row(y) % 2 == 0) == even).unwrap();
    let even = pixel(parity(true));
    let odd = pixel(parity(false));
    even != odd && (0..200).all(|y| pixel(y) == if row(y) % 2 == 0 { even } else { odd })
}

/// Scrolls the list to several offsets, one frame apart, and returns the image showing the last.
fn scroll_through(presenter: fn() -> PixmapPresenter) -> Pixmap {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, cpu(&probe, presenter));
    for offset in [37.0, 120.0, 13.0, 400.0, 401.0] {
        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset]);
        commit(&mut handle, tx);
        probe.wait(|state| {
            state
                .image
                .as_ref()
                .filter(|image| shows(image, offset))
                .cloned()
        });
    }
    let image = probe.state().image.clone().expect("an image was presented");
    driver.shutdown();
    image
}

#[test]
fn kept_target_and_buffer_age_presenters_show_the_same_scrolled_list() {
    let kept = scroll_through(|| PixmapPresenter::kept([200, 200]));
    let rotated = scroll_through(|| PixmapPresenter::buffer_age(3, [200, 200]));
    assert!(kept == rotated);
}
