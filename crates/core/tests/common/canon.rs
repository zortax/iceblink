//! Drawing transcripts with atlas placement replaced by what the tiles hold.

use std::hash::{Hash, Hasher};

use iceblink::Frame;
use iceblink_testing::capture::{Mirror, transcript};

/// Returns what `frame` draws, with every tile written as a hash of its texels and every page
/// number dropped.
///
/// Two frames that draw the same picture from tiles placed differently in the atlas give the
/// same text.
pub(crate) fn canonical(frame: &Frame, mirror: &Mirror) -> String {
    let text = transcript::drawing(frame, mirror).into_string();
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();

    while let Some(at) = rest.find("page") {
        out.push_str(&rest[..at]);
        rest = &rest[at + 4..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            out.push_str("page");
            continue;
        }
        let page: u32 = rest[..digits].parse().expect("digits parse");
        rest = &rest[digits..];

        let tile = rest.strip_prefix('[').and_then(|inner| {
            inner
                .find(']')
                .map(|end| (&inner[..end], &inner[end + 1..]))
        });
        match tile {
            Some((rect, after)) => {
                out.push_str(&format!("tile#{:016x}", texels(mirror, page, rect)));
                rest = after;
            }
            None => out.push_str("page"),
        }
    }

    out.push_str(rest);
    out
}

/// Returns a hash of the texels of `rect`, written `x, y, width, height`, on a page of `mirror`.
fn texels(mirror: &Mirror, page: u32, rect: &str) -> u64 {
    let numbers: Vec<u32> = rect
        .split(',')
        .map(|part| part.trim().parse().expect("a tile rectangle holds numbers"))
        .collect();
    let [x, y, width, height] = numbers[..] else {
        panic!("a tile rectangle has four numbers: {rect}");
    };

    let mut hasher = rustc_hash::FxHasher::default();
    (width, height).hash(&mut hasher);
    let Some(page) = mirror.pages.get(&page) else {
        return hasher.finish();
    };
    for row in y..y + height {
        for column in x..x + width {
            page.texel(column, row).hash(&mut hasher);
        }
    }
    hasher.finish()
}
