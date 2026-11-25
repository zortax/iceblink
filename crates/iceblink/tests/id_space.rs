//! Client-side minting of ids.

mod common;

use std::collections::HashSet;
use std::thread;

use iceblink::{
    AnyId, ClientId, ClipNode, IdSpace, OrderNode, Paint, Transaction, TransformId, TransformNode,
};

#[test]
fn ids_start_at_index_zero_and_generation_one_per_kind() {
    let ids = common::ids();

    let first = ids.mint::<TransformNode>();
    let second = ids.mint::<TransformNode>();
    let clip = ids.mint::<ClipNode>();

    assert_eq!((first.slot(), first.generation()), (0, 1));
    assert_eq!((second.slot(), second.generation()), (1, 1));
    assert_eq!((clip.slot(), clip.generation()), (0, 1));
}

#[test]
fn the_lowest_free_index_is_minted_again_with_the_next_generation() {
    let ids = common::ids();
    let minted: Vec<TransformId> = (0..4).map(|_| ids.mint()).collect();

    ids.free(&[AnyId::Transform(minted[2]), AnyId::Transform(minted[1])]);
    let again = ids.mint::<TransformNode>();
    let after = ids.mint::<TransformNode>();
    let fresh = ids.mint::<TransformNode>();

    assert_eq!((again.slot(), again.generation()), (1, 2));
    assert_eq!((after.slot(), after.generation()), (2, 2));
    assert_eq!((fresh.slot(), fresh.generation()), (4, 1));
}

#[test]
fn freeing_a_stale_or_freed_id_does_nothing() {
    let ids = common::ids();
    let old = ids.mint::<Paint>();
    ids.free(&[old.into(), old.into()]);
    let new = ids.mint::<Paint>();

    ids.free(&[old.into()]);
    let next = ids.mint::<Paint>();

    assert_eq!((new.slot(), new.generation()), (0, 2));
    assert_eq!((next.slot(), next.generation()), (1, 1));
}

#[test]
fn clones_share_their_tables_across_threads() {
    let ids = IdSpace::new(ClientId(7));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let ids = ids.clone();
            thread::spawn(move || {
                (0..250)
                    .map(|_| ids.mint::<TransformNode>().slot())
                    .collect::<Vec<_>>()
            })
        })
        .collect();

    let slots: HashSet<u32> = workers
        .into_iter()
        .flat_map(|worker| worker.join().unwrap())
        .collect();

    assert_eq!(slots, (0..1000).collect());
    assert_eq!(ids.client(), ClientId(7));
}

#[test]
fn only_clones_of_one_space_are_the_same_space() {
    let ids = IdSpace::new(ClientId(7));
    let twin = IdSpace::new(ClientId(7));

    assert!(ids.same(&ids.clone()));
    assert!(!ids.same(&twin));
}

#[test]
fn a_dropped_transaction_frees_its_minted_ids_and_one_taken_apart_does_not() {
    let ids = common::ids();

    let mut dropped = Transaction::new(ids.clone());
    let abandoned = dropped.create_order(OrderNode::default());
    drop(dropped);
    let mut kept = Transaction::new(ids.clone());
    let reused = kept.create_order(OrderNode::default());
    let parts = kept.into_parts();
    let next = ids.mint::<OrderNode>();

    assert_eq!((abandoned.slot(), abandoned.generation()), (0, 1));
    assert_eq!((reused.slot(), reused.generation()), (0, 2));
    assert_eq!(parts.minted, [AnyId::Order(reused)]);
    assert_eq!(next.slot(), 1);
}
