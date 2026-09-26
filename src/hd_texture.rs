//! Decodes the true-colour textures of MTM2 Community Patch 3, PNG and TGA, to RGBA.
//!
//! Not a slice: a helper for `track` and `truck`, which may not use each other. `pod` finds
//! and checks the files but cannot decode them, as it uses nothing but `std`, and a PNG
//! takes the `png` crate.

use crate::pod::{HdFormat, HdTexture};

/// The texture as red, green, blue and alpha bytes, row by row from the top, the order a
/// .RAW is in. `None` if the file is damaged, or its size isn't the one its header gave.
pub fn decode(texture: &HdTexture) -> Option<Vec<u8>> {
    let rgba = match texture.format() {
        HdFormat::Png => decode_png(texture.bytes(), texture.size())?,
        HdFormat::Tga => decode_tga(texture.bytes(), texture.size())?,
    };
    (rgba.len() == texture.size() * texture.size() * 4).then_some(rgba)
}

fn decode_png(bytes: &[u8], size: usize) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // Palettes and small bit depths become 8-bit colour, 16-bit channels become 8-bit.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    if (frame.width as usize, frame.height as usize) != (size, size) {
        return None;
    }
    let pixels = &pixels[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[red, green, blue]| [red, green, blue, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[grey, alpha]| [grey, grey, grey, alpha])
            .collect(),
        png::ColorType::Grayscale => pixels
            .iter()
            .flat_map(|&grey| [grey, grey, grey, 255])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Some(rgba)
}

/// A true-colour TGA (Truevision TGA 2.0): an 18-byte header, an image ID of the length
/// in its first byte, then pixels as blue, green, red and, at 32 bits, alpha. Type 2 is
/// stored as it is. Type 10 is in packets, each a byte whose low 7 bits are one less than
/// the packet's length in pixels and whose top bit says whether one pixel follows, to be
/// repeated, or that many different ones. `pod` has already refused other kinds.
fn decode_tga(bytes: &[u8], size: usize) -> Option<Vec<u8>> {
    let header = bytes.get(..18)?;
    let (id_length, image_type, bits, descriptor) = (header[0], header[2], header[16], header[17]);
    let bytes_per_pixel = bits as usize / 8;
    let mut at = 18 + id_length as usize;
    let next_pixel = |at: &mut usize| -> Option<[u8; 4]> {
        let stored = bytes.get(*at..*at + bytes_per_pixel)?;
        *at += bytes_per_pixel;
        let alpha = if bytes_per_pixel == 4 { stored[3] } else { 255 };
        Some([stored[2], stored[1], stored[0], alpha])
    };

    let count = size * size;
    let mut pixels = Vec::with_capacity(count);
    while pixels.len() < count {
        if image_type == 2 {
            pixels.push(next_pixel(&mut at)?);
            continue;
        }
        let packet = *bytes.get(at)?;
        at += 1;
        let length = (packet & 0x7f) as usize + 1;
        if packet & 0x80 != 0 {
            let repeated = next_pixel(&mut at)?;
            pixels.extend(std::iter::repeat_n(repeated, length));
        } else {
            for _ in 0..length {
                pixels.push(next_pixel(&mut at)?);
            }
        }
    }
    pixels.truncate(count);

    // Rows run from the bottom unless bit 5 of the descriptor says the top, and from the
    // left unless bit 4 says the right.
    let (from_top, from_right) = (descriptor & 0x20 != 0, descriptor & 0x10 != 0);
    let mut rgba = Vec::with_capacity(count * 4);
    for row in 0..size {
        let stored_row = if from_top { row } else { size - 1 - row };
        for column in 0..size {
            let stored_column = if from_right {
                size - 1 - column
            } else {
                column
            };
            rgba.extend(pixels[stored_row * size + stored_column]);
        }
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real PNG, encoded by the same crate.
    fn encoded(size: u32, color: png::ColorType, pixels: &[u8]) -> HdTexture {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, size, size);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
        writer.finish().unwrap();
        HdTexture::parse_png(&bytes).unwrap()
    }

    #[test]
    fn decodes_rgba_and_rgb_png_to_rgba() {
        let rgba: Vec<u8> = (0..32 * 32).flat_map(|n| [n as u8, 1, 2, 3]).collect();
        assert_eq!(
            decode(&encoded(32, png::ColorType::Rgba, &rgba)),
            Some(rgba)
        );

        let rgb: Vec<u8> = (0..32 * 32).flat_map(|n| [n as u8, 1, 2]).collect();
        let decoded = decode(&encoded(32, png::ColorType::Rgb, &rgb)).unwrap();
        assert_eq!(&decoded[..8], [0, 1, 2, 255, 1, 1, 2, 255]);
        assert_eq!(decoded.len(), 32 * 32 * 4);
    }

    /// A 32 x 32 TGA of the given type and descriptor, whose pixel at column `x` of row
    /// `y` as stored has red `x` and green `y`.
    fn tga(image_type: u8, descriptor: u8, bits: u8) -> HdTexture {
        let mut bytes = vec![3, 0, image_type, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 32, 0];
        bytes.extend([bits, descriptor]);
        bytes.extend(b"abc");
        for y in 0..32u8 {
            for x in 0..32u8 {
                let pixel = [9, y, x, 200];
                if image_type == 10 {
                    bytes.push(0); // A packet of one different pixel.
                }
                bytes.extend(&pixel[..bits as usize / 8]);
            }
        }
        HdTexture::parse_tga(&bytes).unwrap()
    }

    #[test]
    fn decodes_tga_stored_either_way_up() {
        // From the top: row 0 as stored is the top row.
        let top = decode(&tga(2, 0x20, 32)).unwrap();
        assert_eq!(&top[..4], [0, 0, 9, 200]);
        assert_eq!(&top[(32 + 5) * 4..][..4], [5, 1, 9, 200]);
        // From the bottom, the usual way: row 0 as stored is the bottom row.
        let bottom = decode(&tga(2, 0, 24)).unwrap();
        assert_eq!(&bottom[..4], [0, 31, 9, 255]);
        // Run-length encoded, the same pixels.
        assert_eq!(decode(&tga(10, 0x20, 32)).unwrap(), top);
    }

    #[test]
    fn decodes_a_run_of_one_repeated_pixel() {
        let mut bytes = vec![0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 32, 0, 24, 0x20];
        for _ in 0..8 {
            bytes.extend([0xff, 30, 20, 10]); // 128 pixels of red 10, green 20, blue 30.
        }
        let rgba = decode(&HdTexture::parse_tga(&bytes).unwrap()).unwrap();
        assert!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .all(|&pixel| pixel == [10, 20, 30, 255])
        );

        // One packet short.
        bytes.truncate(bytes.len() - 4);
        assert_eq!(decode(&HdTexture::parse_tga(&bytes).unwrap()), None);
    }

    #[test]
    fn a_damaged_png_decodes_to_nothing() {
        let rgb = vec![7; 32 * 32 * 3];
        let whole = encoded(32, png::ColorType::Rgb, &rgb);
        let cut = HdTexture::parse_png(&whole.bytes()[..whole.bytes().len() / 2]).unwrap();
        assert_eq!(decode(&cut), None);
    }
}
