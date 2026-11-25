//! Written texels converted into premultiplied rows.

use iceblink::ImageFormat;

/// Writes the rows of `source` into the rectangle `rect` of `target`, an image `width` texels
/// wide with four bytes per texel.
///
/// `source` starts at the rectangle's first texel and holds `stride` bytes per row, in `format`.
/// Premultiplied texels are copied, straight ones are premultiplied, and alpha-only ones become
/// black at their alpha.
pub(crate) fn convert_rows(
    format: ImageFormat,
    source: &[u8],
    stride: usize,
    rect: [usize; 4],
    width: usize,
    target: &mut [u8],
) {
    let [x, y, columns, rows] = rect;
    for row in 0..rows {
        let from = &source[row * stride..];
        let at = ((y + row) * width + x) * 4;
        let into = &mut target[at..at + columns * 4];
        match format {
            ImageFormat::RGBA8 => {
                let (texels, _) = from.as_chunks::<4>();
                let (outs, _) = into.as_chunks_mut::<4>();
                for (texel, out) in texels.iter().zip(outs) {
                    let alpha = u32::from(texel[3]);
                    for channel in 0..3 {
                        out[channel] = ((u32::from(texel[channel]) * alpha + 127) / 255) as u8;
                    }
                    out[3] = texel[3];
                }
            }
            ImageFormat::A8 => {
                for (alpha, out) in from.iter().zip(into.as_chunks_mut::<4>().0) {
                    *out = [0, 0, 0, *alpha];
                }
            }
            _ => into.copy_from_slice(&from[..columns * 4]),
        }
    }
}
