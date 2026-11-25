//! Device buffers and table textures.

pub(crate) mod chunks;
pub(crate) mod instances;
pub(crate) mod slots;
pub(crate) mod vectors;

#[cfg(test)]
mod tests {
    #[test]
    fn the_shaders_divide_by_the_width_the_tables_are_allocated_at() {
        let source = iceblink_wgsl::module("quad").expect("the quad module exists");
        let width = crate::upload::TEXELS_WIDE;
        let declared = format!("const cr_TABLE_TEXELS_WIDE: u32 = {width}u;");
        assert!(
            source.contains(&declared),
            "common.wgsl does not declare `{declared}`"
        );
    }
}
