//! Creating, configuring and destroying outputs.

use iceblink::OutputConfig;
use iceblink::OutputId;
use iceblink::geom::{Point, Rect, Size};

use crate::compositor::CompositorImpl;
use crate::output::{Output, bounds_of};

impl CompositorImpl {
    pub(crate) fn add_output(&mut self, config: OutputConfig) -> OutputId {
        let id = OutputId(self.next_output);
        self.next_output += 1;
        self.outputs.insert(id, Output::new(config));
        id
    }

    pub(crate) fn reconfigure_output(&mut self, id: OutputId, config: OutputConfig) {
        let Some(output) = self.outputs.get_mut(&id) else {
            return;
        };

        let old = output.config;
        output.config = config;
        output.pending.set_bound(Some(bounds_of(config)));
        output.pending.clip_to(bounds_of(config));
        output.driven.set_bound(Some(bounds_of(config)));
        output.driven.clip_to(bounds_of(config));

        if old.scale != config.scale || old.opaque != config.opaque {
            output.damage_all();
            return;
        }

        // The pixels the new size adds: a band on the right and a band at the bottom.
        let [old_width, old_height] = old.size.map(|side| side.min(i32::MAX as u32) as i32);
        let [width, height] = config.size.map(|side| side.min(i32::MAX as u32) as i32);
        if width > old_width {
            output.damage(Rect::new(
                Point::new(old_width, 0),
                Size::new(width - old_width, height),
            ));
        }
        if height > old_height {
            output.damage(Rect::new(
                Point::new(0, old_height),
                Size::new(width, height - old_height),
            ));
        }
    }

    pub(crate) fn remove_output(&mut self, id: OutputId) {
        if let Some(mut output) = self.outputs.remove(&id) {
            output.pins.release(&mut self.content);
            self.release_resident(&output.renderer.resident);
        }
    }
}
