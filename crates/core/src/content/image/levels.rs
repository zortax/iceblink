//! The levels of detail of an image on a page of its own.

/// Returns the levels of detail below premultiplied texels of `size`, down to one texel.
///
/// Each level is half the size of the one above, rounded down and at least one texel, and each
/// of its texels is the mean of the two by two texels above it. An odd last row or column is
/// left out, except that a side of one texel is read twice.
pub(crate) fn levels_below(texels: &[u8], size: [u32; 2]) -> Vec<Vec<u8>> {
    let mut levels: Vec<Vec<u8>> = Vec::new();
    let mut above = size;
    while above[0] > 1 || above[1] > 1 {
        let source = levels.last().map_or(texels, Vec::as_slice);
        let level = halve(source, above);
        levels.push(level);
        above = half(above);
    }
    levels
}

/// Returns the texels of each level that a change of `rect` in level 0 changes, as
/// `[x, y, width, height]` per level from level 1, for an image of `size`.
///
/// The list ends at the first level the change leaves untouched.
pub(crate) fn changed_below(rect: [u32; 4], size: [u32; 2]) -> Vec<[u32; 4]> {
    let mut changed = Vec::new();
    let [mut x0, mut y0] = [rect[0], rect[1]];
    let [mut x1, mut y1] = [rect[0] + rect[2], rect[1] + rect[3]];
    let mut above = size;
    while above[0] > 1 || above[1] > 1 {
        let level = half(above);
        x0 /= 2;
        y0 /= 2;
        x1 = x1.div_ceil(2).min(level[0]);
        y1 = y1.div_ceil(2).min(level[1]);
        if x0 >= x1 || y0 >= y1 {
            break;
        }
        changed.push([x0, y0, x1 - x0, y1 - y0]);
        above = level;
    }
    changed
}

fn half(size: [u32; 2]) -> [u32; 2] {
    [(size[0] / 2).max(1), (size[1] / 2).max(1)]
}

fn halve(source: &[u8], size: [u32; 2]) -> Vec<u8> {
    let [width, height] = size.map(|side| side as usize);
    let [out_width, out_height] = half(size).map(|side| side as usize);
    let mut out = vec![0u8; out_width * out_height * 4];
    for y in 0..out_height {
        for x in 0..out_width {
            let mut sums = [0u32; 4];
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let sx = (2 * x + dx).min(width - 1);
                let sy = (2 * y + dy).min(height - 1);
                let at = (sy * width + sx) * 4;
                for (sum, texel) in sums.iter_mut().zip(&source[at..at + 4]) {
                    *sum += u32::from(*texel);
                }
            }
            let at = (y * out_width + x) * 4;
            for (out, sum) in out[at..at + 4].iter_mut().zip(sums) {
                *out = ((sum + 2) / 4) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{changed_below, levels_below};

    #[test]
    fn a_chain_ends_at_one_texel_and_averages_blocks() {
        let mut texels = vec![0u8; 4 * 2 * 4];
        texels[..4].copy_from_slice(&[255, 255, 255, 255]);
        let levels = levels_below(&texels, [4, 2]);
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].len(), 2 * 4);
        assert_eq!(&levels[0][..4], &[64, 64, 64, 64]);
        assert_eq!(levels[1], vec![32, 32, 32, 32]);
    }

    #[test]
    fn a_change_reaches_the_texels_above_it_on_every_level() {
        assert_eq!(
            changed_below([5, 0, 1, 1], [8, 8]),
            vec![[2, 0, 1, 1], [1, 0, 1, 1], [0, 0, 1, 1]]
        );
    }

    #[test]
    fn a_change_to_a_dropped_column_reaches_no_level() {
        assert!(changed_below([4, 0, 1, 1], [5, 1]).is_empty());
    }
}
