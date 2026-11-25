//! Commits from several threads apply in the order `commit` was called across all handles.

mod common;

use std::sync::Mutex;
use std::thread;

use iceblink::{Anchor, Matrix4, TransformId, TransformNode};
use iceblink::{ClientEvent, CommitId, CompositorClient};

use crate::common::thread::scene::{commit, spawn};

const THREADS: usize = 4;
const COMMITS: usize = 50;

fn node(parent: Option<TransformId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

/// Each commit creates a transform node under the one the previous commit created, so a commit
/// applied before its predecessor refers to a node that is not live and is rejected.
#[test]
fn commits_from_several_threads_apply_in_the_order_they_were_called() {
    let (driver, mut handle) = spawn();
    let (client, _) = handle.register_client();
    let mut tx = handle.transaction(client);
    let first = tx.create_transform(node(None));
    commit(&mut handle, tx);

    let chain: Mutex<(TransformId, Vec<CommitId>)> = Mutex::new((first, Vec::new()));
    thread::scope(|scope| {
        for _ in 0..THREADS {
            let mut handle = handle.clone();
            let chain = &chain;
            scope.spawn(move || {
                for _ in 0..COMMITS {
                    // The lock orders the calls.
                    let mut chain = chain.lock().unwrap();
                    let mut tx = handle.transaction(client);
                    chain.0 = tx.create_transform(node(Some(chain.0)));
                    let id = handle.commit(tx);
                    chain.1.push(id);
                }
            });
        }
    });

    let (_, called) = chain.into_inner().unwrap();
    assert_eq!(called.len(), THREADS * COMMITS);
    handle.sync();
    let mut events = Vec::new();
    handle.drain_events(client, &mut events);
    let applied: Vec<CommitId> = events
        .iter()
        .map(|event| match event {
            ClientEvent::Committed { commit, .. } => *commit,
            ClientEvent::Rejected { commit, error } => {
                panic!("commit {commit:?} was applied out of order: {error}")
            }
            other => panic!("unexpected event {other:?}"),
        })
        .collect();
    assert_eq!(
        applied, called,
        "the commits apply in the order they were called"
    );
    driver.shutdown();
}
