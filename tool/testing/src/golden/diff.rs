//! The first difference between two blocks of text.

/// Describes the first line where `expected` and `actual` differ, with two lines of context and
/// the line counts of both, or returns `None` when they are equal.
pub fn first_difference(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }

    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let at = expected_lines
        .iter()
        .zip(&actual_lines)
        .position(|(left, right)| left != right)
        .unwrap_or(expected_lines.len().min(actual_lines.len()));

    let mut report = format!(
        "goldens differ at line {} ({} expected lines, {} actual)\n",
        at + 1,
        expected_lines.len(),
        actual_lines.len()
    );
    for index in at.saturating_sub(2)..at {
        if let Some(line) = expected_lines.get(index) {
            report.push_str(&format!("  {:>4} | {line}\n", index + 1));
        }
    }
    report.push_str(&format!(
        "- {:>4} | {}\n",
        at + 1,
        expected_lines.get(at).copied().unwrap_or("<end of file>")
    ));
    report.push_str(&format!(
        "+ {:>4} | {}\n",
        at + 1,
        actual_lines.get(at).copied().unwrap_or("<end of file>")
    ));
    Some(report)
}
