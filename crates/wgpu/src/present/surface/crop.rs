//! The extent of a surface that may be larger than the output.

/// How many times the needed area a kept extent may cover.
const SHRINK_SLACK: u64 = 2;

/// Returns the extent a cropping surface takes for an output of `used` texels.
///
/// The `current` extent is kept while `used` fits in it and it is at most twice the area of
/// `used` rounded up to size classes. Otherwise the extent is `used` rounded up to size classes,
/// at most `largest` on each axis.
pub(crate) fn cropped_extent(used: [u32; 2], current: Option<[u32; 2]>, largest: u32) -> [u32; 2] {
    let class = |length: u32| {
        (crate::target::scene_texture::size_class(length as i32) as u32)
            .min(largest)
            .max(length)
    };
    let classed = [class(used[0]), class(used[1])];
    let area = |extent: [u32; 2]| u64::from(extent[0]) * u64::from(extent[1]);
    match current {
        Some(current)
            if used[0] <= current[0]
                && used[1] <= current[1]
                && area(current) <= SHRINK_SLACK * area(classed) =>
        {
            current
        }
        _ => classed,
    }
}
