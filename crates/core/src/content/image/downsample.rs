//! Images reduced to fit a page.

/// Returns the whole factor that shrinks the larger side of `size` to at most `limit`.
pub(crate) fn factor_for(size: [u32; 2], limit: u32) -> u32 {
    size[0].max(size[1]).div_ceil(limit.max(1)).max(1)
}

/// Returns the extent of an image of `size` reduced by `factor`, rounded up.
pub(crate) fn reduced(size: [u32; 2], factor: u32) -> [u32; 2] {
    [size[0].div_ceil(factor), size[1].div_ceil(factor)]
}

/// Returns the premultiplied texels of an image of `size` reduced by `factor`.
///
/// Each texel is the mean of the block of `factor` by `factor` texels it covers, cut at the
/// image's edges.
pub(crate) fn downsample(texels: &[u8], size: [u32; 2], factor: u32) -> Vec<u8> {
    let [width, height] = size.map(|side| side as usize);
    let step = factor as usize;
    let [out_width, out_height] = reduced(size, factor).map(|side| side as usize);
    let mut out = Vec::with_capacity(out_width * out_height * 4);
    for block_y in 0..out_height {
        for block_x in 0..out_width {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for y in block_y * step..((block_y + 1) * step).min(height) {
                for x in block_x * step..((block_x + 1) * step).min(width) {
                    let at = (y * width + x) * 4;
                    for channel in 0..4 {
                        sum[channel] += u32::from(texels[at + channel]);
                    }
                    count += 1;
                }
            }
            let count = count.max(1);
            out.extend(sum.map(|total| ((total + count / 2) / count) as u8));
        }
    }
    out
}
