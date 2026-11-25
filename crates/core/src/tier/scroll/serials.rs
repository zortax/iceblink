//! The serials of a scroll node's events, with the compositor delta at each.

use std::collections::VecDeque;

/// How many serials past the last acknowledged one are kept.
const KEPT: usize = 256;

/// The serials issued for a scroll node and the applied compositor delta at each.
///
/// Serials increase by one per event. The delta at a serial is the one its event carried, before
/// its conversion to `f32`, rebased whenever the client acknowledges an earlier serial.
#[derive(Debug, Default)]
pub(crate) struct Serials {
    last: u64,
    kept: VecDeque<(u64, [f64; 2])>,
    /// The newest serial no longer kept, with its delta.
    floor: (u64, [f64; 2]),
}

impl Serials {
    /// Issues the next serial for an event that carries `delta`.
    pub(crate) fn issue(&mut self, delta: [f64; 2]) -> u64 {
        self.last += 1;
        self.kept.push_back((self.last, delta));
        if self.kept.len() > KEPT
            && let Some(oldest) = self.kept.pop_front()
        {
            self.floor = oldest;
        }
        self.last
    }

    /// Returns the delta of the newest serial at or before `serial`.
    ///
    /// A serial older than every kept one gets the delta of the newest serial no longer kept.
    pub(crate) fn delta_at(&self, serial: u64) -> [f64; 2] {
        let at = self.kept.partition_point(|&(kept, _)| kept <= serial);
        match at {
            0 => self.floor.1,
            at => self.kept[at - 1].1,
        }
    }

    /// Drops the serials up to `serial` and rebases the deltas after it on the delta at it.
    pub(crate) fn acknowledge(&mut self, serial: u64) {
        let adopted = self.delta_at(serial);

        while self.kept.front().is_some_and(|&(kept, _)| kept <= serial) {
            self.kept.pop_front();
        }

        for (_, delta) in &mut self.kept {
            delta[0] -= adopted[0];
            delta[1] -= adopted[1];
        }
        self.floor = (serial.min(self.last), [0.0; 2]);
    }

    /// Forgets every delta, after a write that made the compositor delta zero.
    pub(crate) fn reset(&mut self) {
        self.kept.clear();
        self.floor = (self.last, [0.0; 2]);
    }
}
