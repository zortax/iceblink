//! Registering clients does not wait for the frame the driver is drawing, twice in a row.

mod common;

use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use iceblink::{
    BufferAge, Frame, FrameOutcome, MemoryReport, OrderNode, RenderCapabilities, RenderTarget,
};
use iceblink::{CompositorClient, OutputConfig, Renderer};
use iceblink_testing::fakes::NullRenderer;

use crate::common::thread::scene::spawn;

/// Whether a renderer is inside a draw, and whether its draws may finish.
#[derive(Default)]
struct Gate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}

/// A renderer whose draws wait until the gate opens.
struct Gated {
    inner: NullRenderer,
    gate: Arc<Gate>,
}

impl Renderer for Gated {
    fn capabilities(&self) -> RenderCapabilities {
        self.inner.capabilities()
    }

    fn configure(&mut self, target: RenderTarget) {
        self.inner.configure(target);
    }

    fn target(&self) -> Option<RenderTarget> {
        self.inner.target()
    }

    fn target_age(&self) -> BufferAge {
        self.inner.target_age()
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        let mut state = self.gate.state.lock().unwrap();
        state.0 = true;
        self.gate.changed.notify_all();
        let _open = self
            .gate
            .changed
            .wait_while(state, |state| !state.1)
            .unwrap();
        self.inner.draw(frame)
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }
}

#[test]
fn two_registrations_take_the_reserve_while_a_frame_draws() {
    let (driver, mut handle) = spawn();
    let gate = Arc::new(Gate::default());
    let held = Arc::clone(&gate);
    let renderer = Box::new(move || {
        Ok(Box::new(Gated {
            inner: NullRenderer::new(),
            gate: held,
        }) as Box<dyn Renderer>)
    });
    // The output's first frame waits at the gate. The registration has the driver register the
    // next reserve client before it creates the output.
    let (client, _) = handle.register_client();
    let config = OutputConfig {
        size: [100, 100],
        scale: 1.0,
        opaque: true,
    };
    let output = handle.create_output(config, renderer);
    let mut tx = handle.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    handle.commit(tx);
    drop(
        gate.changed
            .wait_while(gate.state.lock().unwrap(), |state| !state.0)
            .unwrap(),
    );

    let (registered, answer) = mpsc::channel();
    let mut registering = handle.clone();
    thread::spawn(move || {
        let first = registering.register_client();
        let second = registering.register_client();
        let _ = registered.send((first, second));
    });
    let answered = answer.recv_timeout(Duration::from_secs(5));

    gate.state.lock().unwrap().1 = true;
    gate.changed.notify_all();
    let ((first, _), (second, _)) =
        answered.expect("two registrations answer while the frame draws");
    assert!(first != client && second != client && first != second);
    driver.shutdown();
}
