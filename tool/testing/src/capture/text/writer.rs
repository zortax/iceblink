//! The indented line writer transcripts are built with.

/// Collects indented lines into one block of text.
///
/// Every line ends with a newline. A newline or carriage return inside a line is escaped as `\n`
/// or `\r`, so no value adds a line of its own.
///
/// ```
/// use iceblink_testing::capture::text::Writer;
///
/// let mut writer = Writer::new();
/// writer.line("frame 1");
/// writer.nested("batches 1", |writer| writer.line("quads 0..1"));
///
/// assert_eq!(writer.finish(), "frame 1\nbatches 1\n  quads 0..1\n");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Writer {
    out: String,
    depth: usize,
}

impl Writer {
    /// How many spaces one level of nesting indents by.
    pub const INDENT: usize = 2;

    /// Returns a writer with nothing written.
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes one line at the current depth.
    pub fn line(&mut self, text: &str) {
        for _ in 0..self.depth * Self::INDENT {
            self.out.push(' ');
        }
        for character in text.chars() {
            match character {
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                other => self.out.push(other),
            }
        }
        self.out.push('\n');
    }

    /// Writes `header`, then `body` one level deeper.
    pub fn nested(&mut self, header: &str, body: impl FnOnce(&mut Self)) {
        self.line(header);
        self.depth += 1;
        body(self);
        self.depth -= 1;
    }

    /// Returns the depth the next line is written at.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Reports whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// Returns the text written.
    pub fn finish(self) -> String {
        self.out
    }
}
