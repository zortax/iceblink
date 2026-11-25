//! Printing of the compositor's log events to standard output.

use std::fmt::Write as _;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// Installs a subscriber that prints every event of level info and above from the compositor's
/// crates, one line each.
pub(crate) fn install() {
    // A subscriber installed before this one keeps its events.
    let _ = tracing::subscriber::set_global_default(Stdout);
}

/// A subscriber that prints events to standard output.
struct Stdout;

impl Subscriber for Stdout {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        *metadata.level() <= Level::INFO && metadata.target().starts_with("iceblink")
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        let metadata = event.metadata();
        println!("{} {}:{}", metadata.level(), metadata.target(), fields.0);
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

/// The fields of one event, printed as `name=value` after the message.
struct Fields(String);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        // Writing into a string cannot fail.
        let _ = if field.name() == "message" {
            write!(self.0, " {value:?}")
        } else {
            write!(self.0, " {}={value:?}", field.name())
        };
    }
}
