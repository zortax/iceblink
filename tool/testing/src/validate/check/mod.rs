//! The checks of one frame, in the order a renderer applies and draws it.

mod batches;
mod capabilities;
mod covers;
mod damage;
mod inspect;
mod instances;
mod keys;
mod mark_plan;
mod notes;
mod objects;
pub(crate) mod rects;
mod resolve;
mod resources;
mod tables;
mod vector;

use iceblink::{Frame, RenderCapabilities};

use crate::validate::known::Known;
use crate::validate::{FrameViolation, Rule};

pub(crate) use self::inspect::Inspect;

/// The violations found so far.
#[derive(Debug, Default)]
pub(crate) struct Report {
    violations: Vec<FrameViolation>,
}

impl Report {
    pub(crate) fn push(&mut self, rule: Rule, detail: impl Into<String>) {
        self.violations.push(FrameViolation::new(rule, detail));
    }
}

/// Applies `frame` to `known` and returns every rule it breaks.
pub(crate) fn run(
    frame: &Frame,
    known: &mut Known,
    caps: Option<&RenderCapabilities>,
) -> Vec<FrameViolation> {
    let mut report = Report::default();
    damage::check(frame, &mut report);
    resources::apply(frame, known, caps, &mut report);
    notes::apply(frame, known, &mut report);
    tables::check(frame, &mut report);
    known.apply_tables(&frame.tables);
    if let Some(caps) = caps {
        capabilities::check(frame, caps, &mut report);
    }

    let mut inspect = Inspect::new(frame, known, report);
    let lanes = resolve::lanes(&mut inspect);
    instances::check(&mut inspect, &lanes);
    objects::check(&mut inspect);
    let depths = batches::check(&mut inspect, &lanes);
    vector::check(&mut inspect);
    mark_plan::check(&mut inspect, &lanes);
    covers::check(&mut inspect, &lanes, &depths);
    let mut report = inspect.report;
    keys::check(frame, known, &mut report);
    report.violations
}
