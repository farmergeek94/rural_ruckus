//! Ground textures: the list of them (.TEX), the images (.RAW) and the palette (.ACT).
//! See `docs/formats/textures.md`.

use super::PodError;

/// The ground textures a track uses, in the order the texture map's indices refer to
/// them. Text: a count on the first line, then one file name per line.
pub fn parse_texture_list(text: &str) -> Result<Vec<String>, PodError> {
    let mut lines = text.lines().map(str::trim);
    let count_line = lines.next().unwrap_or("");
    let count: usize = count_line.parse().map_err(|_| PodError::BadValue {
        field: "texture count".into(),
        line: 1,
        text: count_line.to_string(),
    })?;

    let names: Vec<String> = lines
        .filter(|name| !name.is_empty())
        .take(count)
        .map(str::to_string)
        .collect();
    if names.len() < count {
        return Err(PodError::MissingField {
            field: format!("texture {} of {count}", names.len() + 1),
        });
    }
    Ok(names)
}

/// 256 colours of red, green and blue, one byte each: 768 bytes with no header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette(Vec<[u8; 3]>);

impl Palette {
    pub fn parse(bytes: &[u8]) -> Result<Self, PodError> {
        if bytes.len() != 768 {
            return Err(PodError::BadSize {
                expected: "768 bytes (256 colours)".into(),
                actual: bytes.len(),
            });
        }
        Ok(Self(bytes.as_chunks::<3>().0.to_vec()))
    }

    pub fn color(&self, index: u8) -> [u8; 3] {
        self.0[index as usize]
    }
}

/// A square image of palette indices, one byte per pixel, row by row from the top, with
/// no header: 4096 bytes for the usual 64 x 64.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    size: usize,
    indices: Vec<u8>,
}

impl Texture {
    pub fn parse(bytes: &[u8]) -> Result<Self, PodError> {
        let size = bytes.len().isqrt();
        if size == 0 || size * size != bytes.len() {
            return Err(PodError::BadSize {
                expected: "a square number of bytes".into(),
                actual: bytes.len(),
            });
        }
        Ok(Self {
            size,
            indices: bytes.to_vec(),
        })
    }

    /// Pixels along each side.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Palette index of the pixel in column `x` of row `y`, counting from the top left.
    pub fn index(&self, x: usize, y: usize) -> u8 {
        self.indices[y * self.size + x]
    }

    /// The whole image as red, green, blue and (opaque) alpha bytes, row by row.
    pub fn to_rgba(&self, palette: &Palette) -> Vec<u8> {
        self.indices
            .iter()
            .flat_map(|&index| {
                let [red, green, blue] = palette.color(index);
                [red, green, blue, 255]
            })
            .collect()
    }
}

/// An image of palette indices of any shape, one byte per pixel, row by row from the top,
/// with no header, as the cockpit's pictures are. The file does not give the size: the
/// file that names the picture does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    width: usize,
    height: usize,
    indices: Vec<u8>,
}

impl Picture {
    /// An error unless there are exactly `width` x `height` bytes.
    pub fn parse(bytes: &[u8], width: usize, height: usize) -> Result<Self, PodError> {
        if width == 0 || height == 0 || width.checked_mul(height) != Some(bytes.len()) {
            return Err(PodError::BadSize {
                expected: format!("{width} x {height} bytes"),
                actual: bytes.len(),
            });
        }
        Ok(Self {
            width,
            height,
            indices: bytes.to_vec(),
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Palette index of the pixel in column `x` of row `y`, counting from the top left.
    pub fn index(&self, x: usize, y: usize) -> u8 {
        self.indices[y * self.width + x]
    }

    /// Every pixel's palette index, row by row from the top.
    pub fn indices(&self) -> &[u8] {
        &self.indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_as_wide_and_high_as_it_is_told() {
        let picture = Picture::parse(&[1, 2, 3, 4, 5, 6], 3, 2).unwrap();
        assert_eq!((picture.width(), picture.height()), (3, 2));
        assert_eq!(picture.index(2, 0), 3);
        assert_eq!(picture.index(0, 1), 4);
        assert!(Picture::parse(&[1, 2, 3, 4, 5], 3, 2).is_err());
        assert!(Picture::parse(&[], 0, 0).is_err());
        assert!(Picture::parse(&[1], usize::MAX, 2).is_err());
    }

    #[test]
    fn reads_the_texture_list() {
        let names = parse_texture_list("2\r\nGRASS.RAW\r\nROAD01.RAW\r\n").unwrap();
        assert_eq!(names, ["GRASS.RAW", "ROAD01.RAW"]);
    }

    #[test]
    fn a_short_or_headless_texture_list_is_an_error() {
        assert!(matches!(
            parse_texture_list("3\nGRASS.RAW\n"),
            Err(PodError::MissingField { .. })
        ));
        assert!(matches!(
            parse_texture_list("GRASS.RAW\n"),
            Err(PodError::BadValue { line: 1, .. })
        ));
    }

    #[test]
    fn colours_a_texture_through_the_palette() {
        let mut bytes = vec![0; 768];
        bytes[3..6].copy_from_slice(&[10, 20, 30]);
        bytes[765..].copy_from_slice(&[7, 8, 9]);
        let palette = Palette::parse(&bytes).unwrap();
        assert!(Palette::parse(&bytes[1..]).is_err());

        let texture = Texture::parse(&[0, 1, 255, 1]).unwrap();
        assert_eq!(texture.size(), 2);
        assert_eq!(texture.index(0, 1), 255);
        assert_eq!(
            texture.to_rgba(&palette),
            [0, 0, 0, 255, 10, 20, 30, 255, 7, 8, 9, 255, 10, 20, 30, 255]
        );
        assert!(Texture::parse(&[0; 5]).is_err());
    }
}
