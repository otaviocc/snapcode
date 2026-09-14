// SPDX-License-Identifier: MIT
//! Encodes a [`Raster`] to PNG bytes.

use crate::backend::raster::Raster;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("failed to encode PNG: {0}")]
    Png(#[from] png::EncodingError),
}

pub fn to_png(raster: &Raster, dpi: u32) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, raster.width, raster.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let ppm = ((dpi as f64) / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&raster.pixels)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    fn raster(width: u32, height: u32, color: Rgba) -> Raster {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            pixels.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
        Raster {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn encodes_a_readable_png() {
        let source = raster(4, 3, Rgba::rgb(193, 95, 60));
        let bytes = to_png(&source, 144).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "missing PNG signature");

        let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (4, 3));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(&buf[..4], &[193, 95, 60, 255]);
    }

    #[test]
    fn writes_dpi_metadata() {
        let source = raster(2, 2, Rgba::WHITE);
        for (dpi, expected_ppm) in [(72, 2835), (144, 5669), (288, 11339)] {
            let bytes = to_png(&source, dpi).unwrap();
            let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
            let reader = decoder.read_info().unwrap();
            let dims = reader.info().pixel_dims.unwrap();
            assert_eq!(dims.unit, png::Unit::Meter);
            assert_eq!(dims.xppu, expected_ppm, "wrong ppm for {dpi} dpi");
            assert_eq!(dims.yppu, expected_ppm);
        }
    }

    #[test]
    fn preserves_transparency() {
        let source = raster(2, 2, Rgba::new(10, 20, 30, 128));
        let bytes = to_png(&source, 72).unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buf).unwrap();
        assert_eq!(&buf[..4], &[10, 20, 30, 128]);
    }

    #[test]
    fn encoding_is_deterministic() {
        let source = raster(8, 8, Rgba::rgb(1, 2, 3));
        assert_eq!(to_png(&source, 144).unwrap(), to_png(&source, 144).unwrap());
    }
}
