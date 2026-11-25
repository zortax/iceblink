//! Rules on queued gestures and the routing a client predicts for them.

use iceblink::ClientEvent;
use iceblink::{GesturePhase, ScrollPhysics, ScrollRouting};

use crate::conformance::client::Rig;
use crate::conformance::client::stage::ClientStage;

/// A gesture routed as the client predicted sends nothing, and one routed otherwise comes back
/// with the routing the compositor chose and wakes the client.
pub fn a_gesture_routed_otherwise_than_predicted_comes_back(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    let woken = stage.woken();

    let handled = stage.to(node);
    stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Begin, handled);
    stage.driver.sync();
    let rerouted = |events: Vec<ClientEvent>| -> Vec<(ScrollRouting, ScrollRouting)> {
        events
            .into_iter()
            .filter_map(|event| match event {
                ClientEvent::Rerouted {
                    predicted, routing, ..
                } => Some((predicted, routing)),
                _ => None,
            })
            .collect()
    };
    assert!(
        rerouted(stage.events()).is_empty(),
        "a gesture routed as predicted sends nothing"
    );
    assert_eq!(
        stage.woken(),
        woken,
        "a gesture routed as predicted wakes nothing"
    );

    stage.gesture(
        [100.0, 100.0],
        [0.0, 10.0],
        GesturePhase::Begin,
        ScrollRouting::Unhandled,
    );
    stage.driver.sync();
    assert_eq!(
        rerouted(stage.events()),
        [(ScrollRouting::Unhandled, handled)],
        "a gesture routed otherwise comes back with the compositor's routing"
    );
    assert_eq!(
        stage.woken(),
        woken + 1,
        "the misprediction wakes the client"
    );
}
