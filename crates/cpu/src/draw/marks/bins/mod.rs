//! The bins of union mark items: the coverage of each quarter of every pixel, eight bits each.

mod fill;

use std::collections::HashMap;

use iceblink::MarkPlan;
use iceblink::geom::{Device, Point, Rect};

use crate::draw::Scene;
use crate::draw::marks::MarkInstance;
use crate::math::Floor as _;
use crate::target::{from_byte, to_byte};

/// The coverage of one union instance over the device pixels its bin holds.
#[derive(Clone, Debug)]
pub(crate) struct Bin {
    region: Rect<i32, Device>,
    quarters: Vec<[u8; 4]>,
}

impl Bin {
    /// Returns the mean coverage of the quarters of the device pixel that holds `device`, or no
    /// coverage outside the bin.
    pub(crate) fn coverage(&self, device: [f32; 2]) -> f32 {
        let pixel = Point::new(device[0].floored() as i32, device[1].floored() as i32);
        if !self.region.contains(pixel) {
            return 0.0;
        }
        let at = self.index(pixel.x, pixel.y);
        self.quarters[at]
            .iter()
            .map(|quarter| from_byte(*quarter))
            .sum::<f32>()
            * 0.25
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y - self.region.top()) as usize * self.region.width() as usize
            + (x - self.region.left()) as usize
    }

    // Keeps the larger of each quarter and `quarters` at pixel `(x, y)`.
    fn keep_largest(&mut self, x: i32, y: i32, quarters: [f32; 4]) {
        let at = self.index(x, y);
        let held = &mut self.quarters[at];
        for (held, quarter) in held.iter_mut().zip(quarters) {
            *held = to_byte(from_byte(*held).max(quarter));
        }
    }

    // Adds `coverage` to every quarter of pixel `(x, y)`.
    fn add(&mut self, x: i32, y: i32, coverage: f32) {
        let at = self.index(x, y);
        for held in &mut self.quarters[at] {
            *held = to_byte(from_byte(*held) + coverage);
        }
    }
}

/// The bins of a frame's union instances, by their position in the marks lane.
#[derive(Debug, Default)]
pub(crate) struct Bins {
    bins: HashMap<u32, Bin>,
}

impl Bins {
    /// Replaces the bins with one for every bin of `plan`, overflow included.
    ///
    /// A planned bin whose instance is missing or has no payload is left out.
    pub(crate) fn fill(&mut self, scene: &Scene<'_>, plan: &MarkPlan, marks: &[MarkInstance]) {
        self.bins.clear();
        for planned in plan.bins.iter().chain(&plan.overflow) {
            let Some(instance) = marks.get(planned.item as usize) else {
                continue;
            };
            let Some(payload) = &instance.payload else {
                continue;
            };
            let bin = fill::filled(scene, planned, &instance.item, payload);
            self.bins.insert(planned.item, bin);
        }
    }

    /// Returns the bin of the instance at `position`, if it has one.
    pub(crate) fn get(&self, position: u32) -> Option<&Bin> {
        self.bins.get(&position)
    }
}
