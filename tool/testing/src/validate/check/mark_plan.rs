//! The mark plan.

use iceblink::geom::{Point, Rect};
use iceblink::{DeviceRect, MAX_MARK_PAGES, MarkBin, MarkFlags};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::rects::{covered, output};
use crate::validate::check::resolve::Lanes;

/// Checks the plan's damage claim, extent and page count, the order and placement of its bins,
/// and that every known union instance has exactly one bin.
pub(crate) fn check(inspect: &mut Inspect<'_>, lanes: &Lanes<'_>) {
    fits_frame(inspect);
    let bins_of = bin_lists(inspect);
    pages(inspect);
    overlaps(inspect);
    overflow(inspect);
    unions(inspect, lanes, &bins_of);
}

/// Checks the plan's damage claim, page extent and page count against the frame.
fn fits_frame(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let plan = &frame.mark_plan;
    if plan.full_damage && !covered(&frame.damage.draw, output(frame)) {
        inspect.push(
            Rule::MarkPlan,
            "the plan claims full damage, and the draw damage misses part of the output",
        );
    }
    if plan.extent[0] > frame.size[0] || plan.extent[1] > frame.size[1] {
        inspect.push(
            Rule::MarkPlan,
            format!(
                "the page extent {:?} exceeds the output {:?}",
                plan.extent, frame.size
            ),
        );
    }
    if plan.pages > MAX_MARK_PAGES + 1 {
        inspect.push(
            Rule::MarkPlan,
            format!("the plan uses {} pages", plan.pages),
        );
    }
}

/// Checks that both bin lists are sorted by item and fit the extent, and returns how many bins
/// each marks source has.
fn bin_lists(inspect: &mut Inspect<'_>) -> Vec<usize> {
    let frame = inspect.frame;
    let plan = &frame.mark_plan;
    let marks = frame.lanes.marks.drawn();

    let mut bins_of = vec![0usize; marks];
    for (list, bins) in [("bin", &plan.bins), ("overflow bin", &plan.overflow)] {
        if bins.windows(2).any(|pair| pair[0].item >= pair[1].item) {
            inspect.push(
                Rule::MarkPlan,
                format!("the {list}s are not sorted by item"),
            );
        }
        for bin in bins {
            match bins_of.get_mut(bin.item as usize) {
                Some(count) => *count += 1,
                None => inspect.push(
                    Rule::MarkPlan,
                    format!("a {list} names marks source {} of {marks}", bin.item),
                ),
            }
            fits(inspect, list, bin);
        }
    }
    bins_of
}

/// Checks that every bin lies on a page the plan uses.
fn pages(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let plan = &frame.mark_plan;
    for bin in &plan.bins {
        if bin.page >= MAX_MARK_PAGES || bin.page >= plan.pages {
            inspect.push(
                Rule::MarkPlan,
                format!(
                    "the bin of item {} lies on page {} of {}",
                    bin.item, bin.page, plan.pages
                ),
            );
        }
    }
}

/// Checks that no two bins overlap on one page.
fn overlaps(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let plan = &frame.mark_plan;
    for (first, a) in plan.bins.iter().enumerate() {
        let clash = plan.bins[first + 1..]
            .iter()
            .find(|b| a.page == b.page && in_page(a).intersects(in_page(b)));
        if let Some(b) = clash {
            inspect.push(
                Rule::MarkPlan,
                format!(
                    "the bins of items {} and {} overlap on page {}",
                    a.item, b.item, a.page
                ),
            );
        }
    }
}

/// Checks that every overflow bin lies at the origin of the last page.
fn overflow(inspect: &mut Inspect<'_>) {
    let frame = inspect.frame;
    let plan = &frame.mark_plan;
    for bin in &plan.overflow {
        if bin.at != [0, 0] || bin.page + 1 != plan.pages {
            inspect.push(
                Rule::MarkPlan,
                format!(
                    "the overflow bin of item {} lies at {:?} on page {} of {}",
                    bin.item, bin.at, bin.page, plan.pages
                ),
            );
        }
    }
}

/// Checks that every known union instance has one bin and every other instance none.
///
/// `bins_of` holds how many bins each marks source has.
fn unions(inspect: &mut Inspect<'_>, lanes: &Lanes<'_>, bins_of: &[usize]) {
    for (position, resolved) in lanes.marks.iter().enumerate() {
        let Some(resolved) = resolved else {
            continue;
        };

        let union = resolved.instance.marks.flags.contains(MarkFlags::UNION);
        let expected = usize::from(union);
        if bins_of[position] != expected {
            inspect.push(
                Rule::MarkPlan,
                format!(
                    "marks source {position} has {} bins, and union is {union}",
                    bins_of[position]
                ),
            );
        }
    }
}

/// Checks that a bin's region fits the page extent.
fn fits(inspect: &mut Inspect<'_>, list: &str, bin: &MarkBin) {
    let extent = inspect.frame.mark_plan.extent;
    let size = bin.region.size;
    let right = i64::from(bin.at[0]) + i64::from(size.width.max(0));
    let bottom = i64::from(bin.at[1]) + i64::from(size.height.max(0));
    if right > i64::from(extent[0]) || bottom > i64::from(extent[1]) {
        inspect.push(
            Rule::MarkPlan,
            format!(
                "the {list} of item {} reaches {right},{bottom} past the extent {:?}",
                bin.item, extent
            ),
        );
    }
}

/// The texels a bin takes on its page.
fn in_page(bin: &MarkBin) -> DeviceRect {
    Rect::new(
        Point::new(bin.at[0] as i32, bin.at[1] as i32),
        bin.region.size,
    )
}
