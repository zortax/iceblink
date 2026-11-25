//! The text transcripts are written in: numbers that render the same way on every machine, and
//! indented lines.

pub mod number;
pub mod writer;

pub use self::writer::Writer;
