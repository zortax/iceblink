//! Packing and unpacking of keys and generations.

mod common;

use iceblink_alloc::arena::{Generation, Key, NamespaceId};
use proptest::prelude::*;

#[test]
fn a_key_and_an_optional_key_are_one_word() {
    assert_eq!(size_of::<Key<()>>(), size_of::<u64>());
    assert_eq!(size_of::<Option<Key<()>>>(), size_of::<u64>());
    assert_eq!(size_of::<Option<Generation>>(), size_of::<Generation>());
}

#[test]
fn generation_zero_is_rejected() {
    assert_eq!(Generation::new(0), None);
    assert_eq!(Generation::FIRST.get(), 1);
    assert_eq!(Key::<()>::from_u64(0), None);
    // Index 7, namespace 3, generation 0.
    assert_eq!(Key::<()>::from_u64(7 | (3 << 48)), None);
}

#[test]
fn the_debug_form_shows_index_generation_and_namespace() {
    let key = Key::<()>::new(9, Generation::FIRST, NamespaceId::new(2));
    assert_eq!(format!("{key:?}"), "Key(9:1@2)");
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn packing_round_trips_and_is_never_zero(
        index in any::<u32>(),
        generation in 1_u16..=u16::MAX,
        namespace in any::<u16>(),
    ) {
        let namespace = NamespaceId::new(namespace);
        let generation = Generation::new(generation).expect("non-zero");
        let key = Key::<()>::new(index, generation, namespace);

        prop_assert_ne!(key.as_u64(), 0);
        prop_assert_eq!(key.index(), index);
        prop_assert_eq!(key.generation(), generation);
        prop_assert_eq!(key.namespace(), namespace);
        prop_assert_eq!(Key::<()>::from_u64(key.as_u64()), Some(key));
    }

    #[test]
    fn successive_generations_increase_until_the_last(raw in 1_u16..=u16::MAX) {
        let generation = Generation::new(raw).expect("non-zero");
        match generation.next() {
            Some(next) => prop_assert!(next > generation),
            None => prop_assert_eq!(generation, Generation::LAST),
        }
    }
}
