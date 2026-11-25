//! Opaque covers: quads that let a renderer skip what lies beneath them.

use iceblink::OpaqueCover;
use smallvec::SmallVec;

/// The most covers a frame holds.
pub(crate) const KEPT: usize = 4;

/// The covers of a frame, as `Frame::opaque_covers` holds them.
pub(crate) type Covers = SmallVec<[OpaqueCover; KEPT]>;

/// Adds `cover` to the held covers, which arrive in draw order.
///
/// Held covers that `cover` contains are dropped, since it hides everything they hide. When the
/// set is full, the smallest of the held covers and `cover` is dropped. A cover with an empty
/// rectangle is ignored.
pub(crate) fn keep(held: &mut Covers, cover: OpaqueCover) {
    if cover.rect.is_empty() {
        return;
    }

    held.retain(|other| !cover.rect.contains_rect(other.rect));
    if held.len() < KEPT {
        held.push(cover);
        return;
    }

    let (smallest, least) = held
        .iter()
        .enumerate()
        .map(|(index, other)| (index, area(other)))
        .min_by_key(|(_, area)| *area)
        .expect("a full set holds covers");
    if area(&cover) >= least {
        held[smallest] = cover;
    }
}

/// Returns the pixels a cover's rectangle holds.
fn area(cover: &OpaqueCover) -> i64 {
    i64::from(cover.rect.size.width.max(0)) * i64::from(cover.rect.size.height.max(0))
}
