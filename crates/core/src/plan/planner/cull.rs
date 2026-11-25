//! Which objects a frame draws.

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;
use iceblink::geom::Edges;

use crate::plan::emission::Instance;
use crate::plan::pixels::output;

/// The test that keeps an object in a frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cull<'a> {
    output: DeviceRect,
    damage: &'a DamageSet,
    /// How far past the damage the filters of the enclosing groups read.
    pub(crate) reach: i32,
}

impl<'a> Cull<'a> {
    pub(crate) fn new(size: [u32; 2], damage: &'a DamageSet) -> Self {
        Self {
            output: output(size),
            damage,
            reach: 0,
        }
    }

    /// Reports whether `instance` is drawn:
    ///
    /// - A union mark is drawn when its ink meets the draw damage on the output, where its bins
    ///   lie.
    /// - Any other instance is drawn as [`Cull::drawn`] decides.
    pub(crate) fn admits(&self, instance: &Instance) -> bool {
        if instance.union {
            self.on_output(instance.ink)
        } else {
            self.drawn(instance.ink)
        }
    }

    /// Reports whether an object of ink `ink` meets the draw damage on the output.
    ///
    /// Inside groups with filters, the ink is tested against the damage grown by the filters'
    /// reach, past the output too.
    pub(crate) fn drawn(&self, ink: DeviceRect) -> bool {
        if self.reach == 0 {
            return self.on_output(ink);
        }
        if self.damage.is_full() {
            return true;
        }
        let grown = Edges::uniform(self.reach);
        self.damage
            .rects()
            .iter()
            .any(|rect| rect.outset(grown).intersects(ink))
    }

    fn on_output(&self, ink: DeviceRect) -> bool {
        ink.intersection(self.output)
            .is_some_and(|shown| self.damage.intersects(shown))
    }
}
