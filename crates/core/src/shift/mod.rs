//! Deciding which scroll offset changes a frame answers with shifts of the kept target.

mod refusal;
mod rule;

pub(crate) use self::refusal::{Refusal, Refusals};

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect};
use iceblink::smallvec::SmallVec;
use iceblink::{DeviceRect, Shift};

use crate::derive::scroll::whole_pixels;
use crate::geometry::bounds::{INK_LIMIT, map};
use crate::output::scroll::{Late, PendingScroll};
use crate::scene::Scene;

/// What deciding a frame's shifts produced.
#[derive(Debug, Default)]
pub(crate) struct Decision {
    pub(crate) shifts: SmallVec<[Shift; 1]>,
    /// Why each scroll change answered with damage failed the shift rule.
    pub(crate) refused: SmallVec<[Refusal; 1]>,
    /// Item inks measured for the damage of refused scrolls.
    pub(crate) measured: u64,
}

/// Decides, for every pending scroll of an output, between a shift and damage.
///
/// `allowed` reports whether the frame may shift at all: the renderer can shift, the request's
/// age is `Kept`, and the kept target holds the previous frame. `root` is the slot of the output's
/// root in `scene`.
///
/// Refused scrolls add their damage to `damage`, and accepted ones add the damage of content that
/// does not move with them. Then `damage` is carried along every shift and gains the bands the
/// shifts expose. Last, the late damage of an accepted scroll is carried by the part of the shift
/// its node moved after the damage was measured.
pub(crate) fn decide(
    scene: Option<&Scene>,
    root: Option<u32>,
    pending: Vec<PendingScroll>,
    allowed: bool,
    output: DeviceRect,
    damage: &mut DamageSet,
) -> Decision {
    let mut decision = Decision::default();
    let mut late: SmallVec<[(Shift, Late); 1]> = SmallVec::new();

    for mut scroll in pending {
        let shift = match (allowed, scene, root) {
            (true, Some(scene), Some(root)) => Some(accept(
                scene,
                root,
                &scroll,
                output,
                &decision.shifts,
                damage,
            )),
            (true, ..) => Some(Err(Refusal::NotDrawn)),
            (false, ..) => None,
        };
        match shift {
            Some(Ok(shift)) => {
                damage.absorb_set(&scroll.still);
                late.extend(scroll.late.drain(..).map(|held| (shift, held)));
                decision.shifts.push(shift);
            }
            refused => {
                let own = scene.filter(|scene| scene.client == scroll.client);
                decision.measured += scroll.moving.absorb_into(own, damage);
                damage.absorb_set(&scroll.still);
                if let Some(Err(refusal)) = refused {
                    decision.refused.push(refusal);
                }
            }
        }
    }

    for shift in &decision.shifts {
        let before = *damage;
        shift.carry_set(&before, damage);
        shift.expose_into(damage);
    }
    for (shift, held) in late {
        carry_rest(shift, &held, damage);
    }

    decision
}

/// Adds the late damage `held` to `damage`, carried by the part of `shift` its node moved after
/// the damage was measured.
///
/// Damage measured at an offset that is not whole pixels away from the shift's start damages the
/// shift's region.
fn carry_rest(shift: Shift, held: &Late, damage: &mut DamageSet) {
    let Some(before) = whole_pixels(held.before) else {
        damage.absorb(shift.region);
        return;
    };

    let rest = [shift.by[0] - before[0], shift.by[1] - before[1]];
    if rest != [0, 0] {
        Shift { by: rest, ..shift }.carry_set(&held.damage, damage);
    }
}

/// Returns the shift that answers `scroll`, or the condition of the shift rule it fails.
fn accept(
    scene: &Scene,
    root: u32,
    scroll: &PendingScroll,
    output: DeviceRect,
    accepted: &[Shift],
    damage: &mut DamageSet,
) -> Result<Shift, Refusal> {
    if scroll.client != scene.client {
        return Err(Refusal::Foreign);
    }
    let by = scroll.by.and_then(whole_pixels).ok_or(Refusal::NotRigid)?;
    if by == [0, 0] {
        return Err(Refusal::Unmoved);
    }
    if !scroll.contained {
        return Err(Refusal::Uncontained);
    }
    if scroll.spreads {
        return Err(Refusal::Spread);
    }

    let region = port_pixels(scene, scroll.scroll)
        .and_then(|port| port.intersection(output))
        .ok_or(Refusal::Port)?;
    if accepted.iter().any(|shift| shift.region.intersects(region)) {
        return Err(Refusal::Overlap);
    }

    let reads = crate::frame::sources::reading(scene, root, output);
    if reads.iter().any(|extent| extent.intersects(region)) {
        return Err(Refusal::Read);
    }

    let mut partial = scroll.partial;
    let still = rule::check(
        scene,
        root,
        &scroll.entries,
        scroll.scroll,
        region,
        &mut partial,
    )?;

    // Where still clips cover the port in part, the moved pixels are drawn again, and the shift
    // carries this damage to where it moves them.
    for rect in partial.pixels(region) {
        damage.absorb(rect);
    }

    for key in still {
        // Content inside the scrolled content that does not move is drawn again where the shift
        // carries it and where it is.
        let Some(record) = scene.chunks.get(&key) else {
            continue;
        };
        for ink in scene.device_inks(&record.derived.inks).items().flatten() {
            damage.absorb(ink);
            damage.absorb(Rect::new(
                Point::new(ink.origin.x + by[0], ink.origin.y + by[1]),
                ink.size,
            ));
        }
    }

    Ok(Shift { region, by })
}

/// Returns the device pixels of a scroll node's port, when its space maps it to whole pixels
/// without turning or scaling it.
pub(crate) fn port_pixels(scene: &Scene, scroll: u32) -> Option<DeviceRect> {
    let record = scene.scrolls.at(scroll)?;
    let space = scene.transforms.at(record.node.space.slot())?;
    space.world.plane_translation()?;

    let mapped = map(&space.world, record.node.port);
    let whole = [mapped.x0, mapped.y0, mapped.x1, mapped.y1]
        .iter()
        .all(|edge| (edge - edge.round()).abs() < 1e-3);
    if !whole {
        return None;
    }
    mapped.pixels(INK_LIMIT)
}
