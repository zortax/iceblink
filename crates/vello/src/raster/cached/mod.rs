//! Encoded paths kept across frames.

mod fingerprint;

use std::sync::Arc;

use rustc_hash::FxHashMap;
use vello::Scene;

pub(crate) use self::fingerprint::Fingerprint;

/// One encoding, and the path it was made from.
struct Entry {
    /// The path, held so its address cannot be reused by another path.
    _path: Arc<kurbo::BezPath>,
    /// The encoded sub-scene, in path space.
    scene: Scene,
    /// The bytes the encoding holds.
    bytes: usize,
    /// The last frame that drew it.
    used: u64,
}

/// The encodings of recently drawn paths, by [`Fingerprint`].
///
/// An encoding is placed again with a copy each frame. Once the encodings hold more than the byte
/// cap, the frame end drops those drawn longest ago, and never one drawn in the frame that ends.
pub(crate) struct Encodings {
    entries: FxHashMap<Fingerprint, Entry>,
    /// The bytes all entries hold.
    bytes: usize,
    /// The bytes past which entries are dropped.
    cap: usize,
    /// How many frames have ended.
    frame: u64,
    /// Lookups that found an encoding.
    hits: u64,
    /// Lookups that encoded.
    misses: u64,
}

impl std::fmt::Debug for Encodings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Encodings")
            .field("entries", &self.entries.len())
            .field("bytes", &self.bytes)
            .field("hits", &self.hits)
            .field("misses", &self.misses)
            .finish()
    }
}

impl Encodings {
    /// The default byte cap.
    pub(crate) const BYTE_CAP: usize = 64 << 20;

    /// Returns an empty cache that holds `cap` bytes before it drops entries.
    pub(crate) fn with_cap(cap: usize) -> Self {
        Self {
            entries: FxHashMap::default(),
            bytes: 0,
            cap,
            frame: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// Returns the encoding of `wanted`, made by `encode` if there is none.
    ///
    /// `path` is the geometry `wanted` was taken from. The cache holds it as long as the
    /// encoding.
    pub(crate) fn get(
        &mut self,
        wanted: Fingerprint,
        path: &Arc<kurbo::BezPath>,
        encode: impl FnOnce(&mut Scene),
    ) -> &Scene {
        let frame = self.frame;
        let entry = match self.entries.entry(wanted) {
            std::collections::hash_map::Entry::Occupied(held) => {
                self.hits += 1;
                held.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(free) => {
                self.misses += 1;
                let mut scene = Scene::new();
                encode(&mut scene);
                let bytes = bytes_of(&scene);
                self.bytes += bytes;
                free.insert(Entry {
                    _path: Arc::clone(path),
                    scene,
                    bytes,
                    used: frame,
                })
            }
        };
        entry.used = frame;
        &entry.scene
    }

    /// Ends a frame, and drops the entries drawn longest ago while the cache holds more than its
    /// cap.
    pub(crate) fn end_frame(&mut self) {
        if self.bytes > self.cap {
            self.evict();
        }
        self.frame += 1;
    }

    /// Drops entries drawn before the current frame, longest ago first, until the cache holds no
    /// more than its cap or no such entry is left.
    fn evict(&mut self) {
        let mut stale: Vec<(u64, Fingerprint)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.used < self.frame)
            .map(|(key, entry)| (entry.used, *key))
            .collect();
        stale.sort_unstable_by_key(|(used, _)| *used);

        for (_, key) in stale {
            if self.bytes <= self.cap {
                break;
            }
            if let Some(entry) = self.entries.remove(&key) {
                self.bytes -= entry.bytes;
            }
        }
    }

    /// Returns how many encodings are held.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns how many lookups found an encoding and how many encoded, since the cache was made.
    pub(crate) fn counts(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

/// Returns the bytes the streams and ramps of `scene` hold.
fn bytes_of(scene: &Scene) -> usize {
    use std::mem::size_of_val;
    let encoding = scene.encoding();
    size_of_val(encoding.path_tags.as_slice())
        + size_of_val(encoding.path_data.as_slice())
        + size_of_val(encoding.draw_tags.as_slice())
        + size_of_val(encoding.draw_data.as_slice())
        + size_of_val(encoding.transforms.as_slice())
        + size_of_val(encoding.styles.as_slice())
        + size_of_val(encoding.resources.color_stops.as_slice())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kurbo::Shape as _;

    use super::{Encodings, Fingerprint};

    fn square(side: f64) -> iceblink::FramePath {
        iceblink::FramePath {
            path: Arc::new(kurbo::Rect::new(0.0, 0.0, side, side).to_path(0.1)),
            transform: kurbo::Affine::IDENTITY,
            brush: kurbo::Affine::IDENTITY,
            fill: None,
            stroke: None,
            clips: Arc::new([]),
            ink: Default::default(),
        }
    }

    fn encode(into: &mut vello::Scene) {
        let shape = kurbo::Rect::new(0.0, 0.0, 8.0, 8.0);
        into.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            peniko::Color::WHITE,
            None,
            &shape,
        );
    }

    fn get(encodings: &mut Encodings, path: &iceblink::FramePath) -> bool {
        let mut encoded = false;
        encodings.get(Fingerprint::of(path, None, None), &path.path, |into| {
            encoded = true;
            encode(into);
        });
        encoded
    }

    #[test]
    fn the_path_an_encoding_was_made_from_is_held_by_the_cache() {
        let mut encodings = Encodings::with_cap(Encodings::BYTE_CAP);
        let path = square(32.0);

        get(&mut encodings, &path);

        assert!(Arc::strong_count(&path.path) > 1);
    }

    #[test]
    fn entries_past_the_byte_cap_go_oldest_first_and_never_one_drawn_this_frame() {
        let squares = [square(8.0), square(16.0), square(24.0)];
        let mut measure = Encodings::with_cap(Encodings::BYTE_CAP);
        get(&mut measure, &squares[0]);
        let mut encodings = Encodings::with_cap(2 * measure.bytes);

        for path in &squares {
            get(&mut encodings, path);
            encodings.end_frame();
        }

        assert_eq!(encodings.len(), 2);
        assert!(
            !get(&mut encodings, &squares[1]),
            "the second square was drawn later and stays"
        );
        assert!(
            get(&mut encodings, &squares[0]),
            "the first square was drawn longest ago"
        );
    }
}
