//! Which ground texture covers each terrain cell (.CLR). See `docs/formats/terrain.md`.

use super::PodError;

/// One 16-bit value per terrain cell, in the same row and column order as the
/// heightmap. Cell `(column, row)` covers the ground from sample `(column, row)` to
/// sample `(column + 1, row + 1)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureMap {
    size: usize,
    cells: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureCell {
    /// Index into the level's texture list (.TEX).
    pub texture: u16,
    /// Bits 12-13 as written: how the texture is mirrored.
    pub mirror: u8,
    /// Bits 14-15 as written: quarter turns.
    pub rotation: u8,
}

impl TextureCell {
    /// Splits a value as written into its parts.
    pub(super) fn from_bits(value: u16) -> Self {
        Self {
            texture: value & 0x0fff,
            mirror: (value >> 12 & 3) as u8,
            rotation: (value >> 14) as u8,
        }
    }

    /// Where in the texture a point of the cell falls. `u` runs from 0 to 1 across the
    /// cell along +X and `v` along +Z. The result is (across, down) in the image, from 0
    /// to 1 measured from its top left corner.
    ///
    /// Unturned, a texture lies with its top edge along the cell's far (+Z) side and its
    /// left edge along the near (x = 0) side. Mirroring comes first: bit 0 flips it top
    /// to bottom, bit 1 left to right. Then come the quarter turns.
    pub fn texture_coords(&self, u: f32, v: f32) -> (f32, f32) {
        let (mut s, mut t) = (u - 0.5, v - 0.5);
        if self.mirror & 1 != 0 {
            t = -t;
        }
        if self.mirror & 2 != 0 {
            s = -s;
        }
        for _ in 0..self.rotation % 4 {
            (s, t) = (-t, s);
        }
        (s + 0.5, 0.5 - t)
    }
}

impl TextureMap {
    /// `size` is the heightmap's, which the texture map has to match.
    pub fn parse(bytes: &[u8], size: usize) -> Result<Self, PodError> {
        if bytes.len() != size * size * 2 {
            return Err(PodError::BadSize {
                expected: format!(
                    "{} bytes (two per cell of a {size} x {size} map)",
                    size * size * 2
                ),
                actual: bytes.len(),
            });
        }
        let cells = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect();
        Ok(Self { size, cells })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// The world repeats, so indices wrap around.
    pub fn cell(&self, column: usize, row: usize) -> TextureCell {
        TextureCell::from_bits(self.cells[(row % self.size) * self.size + column % self.size])
    }

    /// The texture used by the most cells: the open ground that the course is laid over.
    pub fn most_common_texture(&self) -> u16 {
        let mut counts = vec![0usize; 0x1000];
        for value in &self.cells {
            counts[(value & 0x0fff) as usize] += 1;
        }
        (0..counts.len())
            .max_by_key(|&texture| counts[texture])
            .unwrap_or(0) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_index_mirror_and_rotation() {
        let bytes = [0x05, 0x00, 0x07, 0x90, 0x05, 0x00, 0x05, 0x00];
        let map = TextureMap::parse(&bytes, 2).unwrap();
        assert_eq!(
            map.cell(0, 0),
            TextureCell {
                texture: 5,
                mirror: 0,
                rotation: 0
            }
        );
        assert_eq!(
            map.cell(1, 0),
            TextureCell {
                texture: 7,
                mirror: 1,
                rotation: 2
            }
        );
        assert_eq!(map.cell(3, 2), map.cell(1, 0));
        assert_eq!(map.most_common_texture(), 5);
    }

    #[test]
    fn places_textures_on_cells() {
        let cell = |mirror, rotation| TextureCell {
            texture: 0,
            mirror,
            rotation,
        };
        let close = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() + (a.1 - b.1).abs() < 1e-6;

        // Unturned: the cell's origin corner is the image's bottom left, and its +Z side
        // is the image's top.
        assert!(close(cell(0, 0).texture_coords(0.0, 0.0), (0.0, 1.0)));
        assert!(close(cell(0, 0).texture_coords(1.0, 0.0), (1.0, 1.0)));
        assert!(close(cell(0, 0).texture_coords(0.25, 1.0), (0.25, 0.0)));

        // One quarter turn.
        assert!(close(cell(0, 1).texture_coords(0.0, 0.0), (1.0, 1.0)));
        assert!(close(cell(0, 1).texture_coords(1.0, 0.0), (1.0, 0.0)));
        // Four make none.
        assert!(close(
            cell(0, 4).texture_coords(0.3, 0.8),
            cell(0, 0).texture_coords(0.3, 0.8)
        ));

        // Mirrors.
        assert!(close(cell(1, 0).texture_coords(0.25, 1.0), (0.25, 1.0)));
        assert!(close(cell(2, 0).texture_coords(0.25, 1.0), (0.75, 0.0)));
        // Mirroring happens before turning. The other order would give (1, 1) here.
        assert!(close(cell(1, 1).texture_coords(1.0, 0.0), (0.0, 0.0)));
        assert!(close(cell(1, 1).texture_coords(0.0, 0.0), (0.0, 1.0)));
    }

    #[test]
    fn must_match_the_heightmap() {
        assert!(matches!(
            TextureMap::parse(&[0; 8], 3),
            Err(PodError::BadSize { .. })
        ));
    }
}
