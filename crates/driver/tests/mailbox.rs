//! A client's waker is called for the first waking event after it is set, whatever was queued
//! before.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use iceblink::{ClientEvent, CommitId, CommitReport};
use iceblink::{ClientId, CommitError, RejectReason};
use iceblink_driver::{Letter, Mailbox};

fn rejected(client: ClientId) -> Letter {
    Letter {
        client,
        event: ClientEvent::Rejected {
            commit: CommitId(0),
            error: CommitError {
                reason: RejectReason::StaleId,
                command: None,
            },
        },
        wakes: true,
    }
}

#[test]
fn a_new_waker_is_called_for_the_next_waking_event() {
    let client = ClientId(1);
    let mut mailbox = Mailbox::default();
    mailbox.open(client);
    let mut wake = Vec::new();
    mailbox.deliver(&mut vec![rejected(client)], &mut wake);
    assert!(wake.is_empty(), "a client without a waker is not woken");

    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    mailbox.set_waker(
        client,
        Some(Arc::new(move || {
            counted.fetch_add(1, Ordering::Relaxed);
        })),
    );
    let committed = Letter {
        client,
        event: ClientEvent::Committed {
            commit: CommitId(1),
            report: CommitReport::default(),
        },
        wakes: false,
    };
    mailbox.deliver(
        &mut vec![committed, rejected(client), rejected(client)],
        &mut wake,
    );
    for waker in wake.drain(..) {
        waker();
    }
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "the new waker is called once for the next waking events"
    );
}
