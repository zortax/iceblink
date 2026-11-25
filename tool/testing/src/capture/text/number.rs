//! Numbers rendered the same way on every machine.

/// How many decimals a number keeps.
// A ten-thousandth of a pixel is far below anything visible and far above the noise two routes to
// one coordinate accumulate.
pub const DECIMALS: usize = 4;

/// Renders `value` rounded to [`DECIMALS`] decimals, without trailing zeros.
///
/// A negative zero renders as `0`, and non-finite values as `nan`, `inf` and `-inf`.
///
/// ```
/// use iceblink_testing::capture::text::number::float;
///
/// assert_eq!(float(1.0), "1");
/// assert_eq!(float(-0.0), "0");
/// assert_eq!(float(0.1 + 0.2), "0.3");
/// assert_eq!(float(f32::NAN), "nan");
/// ```
pub fn float(value: f32) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-inf"
        } else {
            "inf"
        }
        .to_owned();
    }

    let mut text = format!("{value:.DECIMALS$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" { "0".to_owned() } else { text }
}

/// Renders `[x, y, width, height]` as `rect(x, y, width, height)`.
pub fn rect(values: [f32; 4]) -> String {
    format!(
        "rect({}, {}, {}, {})",
        float(values[0]),
        float(values[1]),
        float(values[2]),
        float(values[3])
    )
}

/// Renders `values` as `[a, b, ...]`.
pub fn list(values: &[f32]) -> String {
    let rendered: Vec<String> = values.iter().copied().map(float).collect();
    format!("[{}]", rendered.join(", "))
}

/// Reports whether every value is zero.
pub fn all_zero(values: &[f32]) -> bool {
    values.iter().all(|value| *value == 0.0)
}
