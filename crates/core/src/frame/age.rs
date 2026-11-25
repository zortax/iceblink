//! The draw and present damage of a frame, by the age of the image it is drawn into.

use iceblink::damage::DamageSet;
use iceblink::{BufferAge, FrameDamage, Shift};

use crate::output::history::PresentHistory;

/// Splits a frame's own damage into what the renderer redraws and what it presents.
///
/// - With a kept target, the frame redraws its damage and presents it with the shift
///   destinations.
/// - With an image `n` frames old, it also redraws the present damage of the `n - 1` frames
///   before, or everything when fewer are remembered.
/// - With an image of unknown contents, it redraws everything.
pub(crate) fn split(
    own: DamageSet,
    shifts: &[Shift],
    age: BufferAge,
    history: &PresentHistory,
) -> FrameDamage {
    match age {
        BufferAge::Kept => {
            let mut present = own;
            for shift in shifts {
                if let Some(destination) = shift.destination() {
                    present.absorb(destination);
                }
            }
            FrameDamage {
                draw: own,
                present,
                driven: DamageSet::new(),
            }
        }
        BufferAge::Known(age) => {
            let older = age.saturating_sub(1) as usize;
            let draw = match history.union(older) {
                Some(mut owed) => {
                    owed.absorb_set(&own);
                    owed
                }
                None => DamageSet::full(),
            };
            FrameDamage {
                draw,
                present: own,
                driven: DamageSet::new(),
            }
        }
        BufferAge::Unknown => FrameDamage {
            draw: DamageSet::full(),
            present: own,
            driven: DamageSet::new(),
        },
    }
}
