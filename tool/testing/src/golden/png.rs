//! Reading and writing PNG files of RGBA images.

use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

use crate::golden::RgbaImage;

/// Reads the 8-bit RGBA PNG at `path`.
///
/// # Errors
///
/// Fails when the file cannot be read or decoded, or holds another colour type or bit depth.
pub fn read_png(path: &Path) -> io::Result<RgbaImage> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info().map_err(io::Error::other)?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).map_err(io::Error::other)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(io::Error::other(format!(
            "{} is {:?} at {:?}, not 8-bit RGBA",
            path.display(),
            info.color_type,
            info.bit_depth
        )));
    }

    pixels.truncate(info.buffer_size());
    Ok(RgbaImage::from_pixels(info.width, info.height, pixels))
}

/// Writes `image` to `path` as an 8-bit RGBA PNG.
///
/// # Errors
///
/// Fails when the file cannot be created or written.
pub fn write_png(path: &Path, image: &RgbaImage) -> io::Result<()> {
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(path)?),
        image.width(),
        image.height(),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(io::Error::other)?;
    writer
        .write_image_data(image.pixels())
        .map_err(io::Error::other)?;
    writer.finish().map_err(io::Error::other)
}
