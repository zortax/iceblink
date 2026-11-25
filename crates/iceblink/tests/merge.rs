//! The capacity merge checked against a direct implementation of the least-waste rule.
//!
//! When a set at capacity absorbs a rectangle that meets none of its rectangles, it merges the
//! pair whose union, together with every rectangle that union grows over, wastes the fewest
//! pixels. The reference below scores every pair with plain `Rect` operations, and the set must
//! end up holding the untouched candidates plus the bounds of the merged closure.

mod common;

use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Rect};
use proptest::prelude::*;

use crate::common::damage::{Xorshift, rect};

/// Returns which candidates the least-waste merge takes: the cheapest pair and every candidate its
/// union grows over.
fn reference(candidates: &[Rect<i32, Device>]) -> Vec<bool> {
    let area = |rect: Rect<i32, Device>| {
        if rect.is_empty() {
            0
        } else {
            i64::from(rect.width()).saturating_mul(i64::from(rect.height()))
        }
    };
    let mut best: Option<(i64, Vec<bool>)> = None;
    for left in 0..candidates.len() {
        for right in (left + 1)..candidates.len() {
            let mut union = candidates[left].union(candidates[right]);
            let mut taken = vec![false; candidates.len()];
            taken[left] = true;
            taken[right] = true;
            let mut grew = true;
            while grew {
                grew = false;
                for (index, held) in taken.iter_mut().enumerate() {
                    if !*held && candidates[index].intersects(union) {
                        union = union.union(candidates[index]);
                        *held = true;
                        grew = true;
                    }
                }
            }
            let covered = taken
                .iter()
                .enumerate()
                .filter(|(_, held)| **held)
                .fold(0_i64, |sum, (index, _)| {
                    sum.saturating_add(area(candidates[index]))
                });
            let waste = area(union).saturating_sub(covered);
            if best.as_ref().is_none_or(|(least, _)| waste < *least) {
                best = Some((waste, taken));
            }
        }
    }
    best.expect("a set at capacity and one more rectangle make a pair")
        .1
}

/// Returns the rectangles a set at capacity should hold after absorbing `incoming`.
fn expected(held: &[Rect<i32, Device>], incoming: Rect<i32, Device>) -> Vec<Rect<i32, Device>> {
    let mut candidates = held.to_vec();
    candidates.push(incoming);
    let taken = reference(&candidates);
    let mut kept: Vec<_> = candidates
        .iter()
        .zip(&taken)
        .filter(|(_, taken)| !**taken)
        .map(|(rect, _)| *rect)
        .collect();
    let merged = candidates
        .iter()
        .zip(&taken)
        .filter(|(_, taken)| **taken)
        .fold(Rect::ZERO, |union, (rect, _)| union.union(*rect));
    kept.push(merged);
    kept
}

/// Returns the rectangles in a fixed order, so two lists compare as sets.
fn sorted(rects: &[Rect<i32, Device>]) -> Vec<[i32; 4]> {
    let mut keys: Vec<_> = rects
        .iter()
        .map(|rect| [rect.left(), rect.top(), rect.width(), rect.height()])
        .collect();
    keys.sort_unstable();
    keys
}

/// Absorbs `stream`, checking each capacity merge against the reference, and returns how many
/// merges it checked.
fn check_stream<const N: usize>(stream: impl IntoIterator<Item = Rect<i32, Device>>) -> usize {
    let mut set = DamageSet::<N>::new();
    let mut checked = 0;
    for input in stream {
        if set.len() == N && !input.is_empty() && !set.intersects(input) {
            let want = expected(set.rects(), input);
            let before = set;
            set.absorb(input);
            assert_eq!(
                sorted(set.rects()),
                sorted(&want),
                "{before:?} absorbing {input:?}"
            );
            checked += 1;
        } else {
            set.absorb(input);
        }
    }
    checked
}

#[test]
fn long_streams_merge_as_the_reference_does() {
    let grid = (0..500).map(|index| rect(15 + index % 24 * 48, 15 + index / 24 * 22, 42, 16));
    assert!(check_stream::<6>(grid) > 400);

    let mut generator = Xorshift(0x0dd_ba11_cafe_f00d);
    let mut checked = 0;
    for _ in 0..200 {
        let stream: Vec<_> = (0..40)
            .map(|_| {
                rect(
                    generator.below(1900),
                    generator.below(1000),
                    1 + generator.below(120),
                    1 + generator.below(40),
                )
            })
            .collect();
        checked += check_stream::<6>(stream.iter().copied());
        checked += check_stream::<4>(stream);
    }
    assert!(checked > 1000, "only {checked} merges were checked");
}

proptest! {
    #[test]
    fn any_stream_merges_as_the_reference_does(
        inputs in prop::collection::vec(
            (0i32..400, 0i32..400, 1i32..60, 1i32..60)
                .prop_map(|(x, y, width, height)| rect(x, y, width, height)),
            6..40,
        ),
    ) {
        check_stream::<6>(inputs);
    }
}

/// Returns a digest of every state a set of capacity `N` passes through while it absorbs random
/// streams of `seed`.
fn digest_of_streams<const N: usize>(seed: u64) -> u64 {
    let mut generator = Xorshift(seed);
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: i32| {
        digest = (digest ^ u64::from(value as u32)).wrapping_mul(0x0100_0000_01b3);
    };
    for _ in 0..300 {
        let mut set = DamageSet::<N>::new();
        for _ in 0..60 {
            let input = rect(
                generator.below(1900),
                generator.below(1000),
                1 + generator.below(300),
                1 + generator.below(120),
            );
            set.absorb(input);
            for key in sorted(set.rects()) {
                key.into_iter().for_each(&mut mix);
            }
            mix(-1);
        }
    }
    digest
}

/// The digest of [`digest_of_streams`] at capacity 6.
const RECORDED_6: u64 = 0x5bad_2f78_faa7_1476;

/// The digest of [`digest_of_streams`] at capacity 4.
const RECORDED_4: u64 = 0x25b0_81e2_eb56_b249;

#[test]
fn random_streams_leave_the_same_rectangles_as_before() {
    // Recorded with an earlier implementation, so a change to what the merge keeps fails here.
    assert_eq!(digest_of_streams::<6>(0x5eed_0fda_4a6e), RECORDED_6);
    assert_eq!(digest_of_streams::<4>(0x5eed_0fda_4a6e), RECORDED_4);
}
