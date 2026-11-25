//! Bins for the coverage of union marks.

use iceblink::{DeviceRect, MAX_MARK_PAGES, MarkBin, MarkPlan};

use crate::naive::content::Shelves;

/// Fills `plan` with a bin for each union marks instance, given as its position in the marks lane
/// and its device ink.
///
/// Each bin holds the instance's whole ink on the output, so the plan is made for full damage.
/// Bins are packed onto pages the size of the output, and the bins that find no room on the first
/// `MAX_MARK_PAGES` pages go to the spare page.
pub(crate) fn plan(plan: &mut MarkPlan, instances: &[(u32, DeviceRect)], output: DeviceRect) {
    plan.clear();
    plan.full_damage = true;
    if instances.is_empty() {
        return;
    }

    let extent = [output.size.width as u32, output.size.height as u32];
    let mut pages: Vec<Shelves> = Vec::new();
    for (item, ink) in instances {
        let Some(region) = ink.intersection(output) else {
            continue;
        };
        let size = [region.size.width as u32, region.size.height as u32];

        let mut placed = None;
        for (page, shelves) in pages.iter_mut().enumerate() {
            if let Some(at) = shelves.place(size) {
                placed = Some((page as u32, at));
                break;
            }
        }
        if placed.is_none() && (pages.len() as u32) < MAX_MARK_PAGES {
            let mut shelves = Shelves::new(extent, 0);
            placed = shelves.place(size).map(|at| (pages.len() as u32, at));
            pages.push(shelves);
        }

        match placed {
            Some((page, at)) => plan.bins.push(MarkBin {
                item: *item,
                region,
                at,
                page,
            }),
            None => plan.overflow.push(MarkBin {
                item: *item,
                region,
                at: [0, 0],
                page: MAX_MARK_PAGES,
            }),
        }
    }

    let highest = plan
        .bins
        .iter()
        .chain(&plan.overflow)
        .map(|bin| bin.page)
        .max();
    plan.pages = highest.map_or(0, |page| page + 1);
    plan.extent = extent;
}
