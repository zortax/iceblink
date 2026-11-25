//! The client suite on the threaded driver, over the reference compositor and over the naive one,
//! and stepped runs that match the in-process driver's.

use iceblink::{Compositor, CompositorClient};
use iceblink_driver::DriverOptions;
use iceblink_driver::InProcess;
use iceblink_driver::thread::ThreadedCompositor;
use iceblink_testing::conformance::client::{Rig, transcript};

fn threaded(compositor: Box<dyn Compositor>) -> Box<dyn CompositorClient> {
    let (driver, _) = ThreadedCompositor::spawn_with(compositor, DriverOptions::stepped());
    Box::new(driver)
}

fn in_process(compositor: Box<dyn Compositor>) -> Box<dyn CompositorClient> {
    Box::new(InProcess::new(compositor, DriverOptions::stepped()))
}

fn core() -> Box<dyn Compositor> {
    Box::new(iceblink_core::CompositorImpl::new())
}

fn naive() -> Box<dyn Compositor> {
    Box::new(iceblink_testing::naive::NaiveCompositor::new())
}

mod over_core {
    use super::*;

    iceblink_testing::client_suite!(Rig {
        compositor: &core,
        client: &threaded,
    });
}

mod over_naive {
    use super::*;

    iceblink_testing::client_suite!(Rig {
        compositor: &naive,
        client: &threaded,
    });
}

#[test]
fn a_stepped_thread_draws_the_frames_and_queues_the_events_of_the_in_process_driver() {
    for compositor in [core as fn() -> Box<dyn Compositor>, naive] {
        let threaded = transcript(&Rig {
            compositor: &compositor,
            client: &threaded,
        });
        let local = transcript(&Rig {
            compositor: &compositor,
            client: &in_process,
        });
        let differs = threaded.iter().zip(&local).position(|(a, b)| a != b);
        assert!(
            differs.is_none() && threaded.len() == local.len(),
            "the transcripts differ at line {differs:?}:\n{:?}\n{:?}",
            differs.map(|at| &threaded[at]),
            differs.map(|at| &local[at]),
        );
    }
}
