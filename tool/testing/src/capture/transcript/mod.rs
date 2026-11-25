//! Frames as stable text for golden files.
//!
//! A transcript resolves every instance through the tables, pages and resident chunks a renderer
//! holds, so it shows what is drawn and never a raw slot. Two frames that draw the same picture
//! with the same batches have the same drawing, whether their instances were resident or
//! transient. Numbers are rounded the same way on every machine, and fields at their default
//! value are left out.

mod draw;
mod header;
mod instance;
mod paint;
mod path;
mod plans;
mod space;
mod work;

use core::fmt;

use iceblink::Frame;

use crate::capture::mirror::Mirror;
use crate::capture::text::Writer;

/// One frame, rendered as text that ends in a newline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcript {
    text: String,
}

impl Transcript {
    /// Returns the text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Converts the transcript into its text.
    pub fn into_string(self) -> String {
        self.text
    }

    /// Returns the lines, without their terminators.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.text.lines()
    }
}

impl fmt::Display for Transcript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

/// Renders the whole of `frame`: its id, output and damage, the work it hands the renderer, and
/// its drawing.
///
/// `mirror` holds what the renderer holds once the frame's work is applied.
pub fn of(frame: &Frame, mirror: &Mirror) -> Transcript {
    whole(frame, &drawn(frame, mirror))
}

/// Renders what `frame` draws: its output, every batch with its resolved instances, the opaque
/// covers and the mark pages.
///
/// `mirror` holds what the renderer holds once the frame's work is applied.
pub fn drawing(frame: &Frame, mirror: &Mirror) -> Transcript {
    only_drawing(frame, &drawn(frame, mirror))
}

/// Renders [`of`] and [`drawing`] of `frame` together.
pub fn both(frame: &Frame, mirror: &Mirror) -> (Transcript, Transcript) {
    let drawn = drawn(frame, mirror);
    (whole(frame, &drawn), only_drawing(frame, &drawn))
}

fn whole(frame: &Frame, drawn: &str) -> Transcript {
    let mut writer = Writer::new();
    writer.line(&format!("frame {}", frame.id.0));
    header::output(&mut writer, frame);
    header::damage(&mut writer, frame);
    work::work(&mut writer, frame);
    let mut text = writer.finish();
    text.push_str(drawn);
    Transcript { text }
}

fn only_drawing(frame: &Frame, drawn: &str) -> Transcript {
    let mut writer = Writer::new();
    header::output(&mut writer, frame);
    let mut text = writer.finish();
    text.push_str(drawn);
    Transcript { text }
}

/// Renders the batches, covers and mark pages at the top level.
fn drawn(frame: &Frame, mirror: &Mirror) -> String {
    let mut writer = Writer::new();
    draw::batches(&mut writer, frame, mirror);
    plans::covers(&mut writer, frame);
    plans::mark_pages(&mut writer, frame);
    writer.finish()
}
