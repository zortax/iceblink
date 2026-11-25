//! The device regions targets cover and the sizes they are allocated at.

use iceblink::geom::{Device, Edges, Point, Rect, Size};

use crate::target::group_pool::GroupPool;
use crate::target::scale::TargetScale;
use crate::target::scene_texture::size_class;

/// Returns the device region a target for `region` covers in an output of `surface`.
///
/// The cover adds the guard around `region` and starts on an even device pixel, moved up and left
/// by `margin`. It is cut to the cover of the whole output unless `region` reaches past the output.
pub(super) fn cover_for(
    region: Rect<i32, Device>,
    surface: Size<i32, Device>,
    margin: i32,
) -> Rect<i32, Device> {
    let guarded = region.outset(Edges::uniform(GroupPool::GUARD));
    let near = Point::new(
        guarded.left().div_euclid(2) * 2 - margin,
        guarded.top().div_euclid(2) * 2 - margin,
    );
    let cover = Rect::from_corners(near, guarded.far_corner());
    // A group's source can reach past the surface, and its target then follows it.
    if !Rect::new(Point::new(0, 0), surface).contains_rect(region) {
        return cover;
    }
    let limit = whole_cover(surface, margin);
    cover.intersection(limit).unwrap_or(limit)
}

/// Returns the device region a target for the whole output covers.
pub(super) fn whole_cover(surface: Size<i32, Device>, margin: i32) -> Rect<i32, Device> {
    Rect::from_corners(
        Point::new(-margin, -margin),
        Point::new(
            surface.width + GroupPool::GUARD,
            surface.height + GroupPool::GUARD,
        ),
    )
}

/// Returns the extent a target for the whole output is allocated at.
pub(super) fn whole_allocated(
    region: Size<i32, Device>,
    margin: i32,
    scale: TargetScale,
) -> Size<i32, Device> {
    allocated_for(whole_cover(region, margin).size, scale)
}

/// Returns the margin [`GroupPool::POISON_ORIGIN`] holds, or zero.
pub(super) fn poison_margin() -> i32 {
    std::env::var(GroupPool::POISON_ORIGIN)
        .ok()
        .and_then(|raw| raw.trim().parse::<i32>().ok())
        .map_or(0, even_margin)
}

/// Returns the magnitude of `margin`, at most 4096 and rounded up to an even number.
pub(super) fn even_margin(margin: i32) -> i32 {
    margin.unsigned_abs().min(1 << 12).next_multiple_of(2) as i32
}

/// Returns the extent a target for `region` is allocated at: the region rounded up to size
/// classes, at `scale`.
pub(super) fn allocated_for(region: Size<i32, Device>, scale: TargetScale) -> Size<i32, Device> {
    let classed: Size<i32, Device> = Size::new(size_class(region.width), size_class(region.height));
    scale.extent(classed)
}
