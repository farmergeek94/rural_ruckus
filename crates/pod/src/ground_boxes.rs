//! Ground boxes (.RA0, .RA1 and .CL0 beside the heightmap): solid blocks standing on the
//! terrain's grid, which make bridges, tunnel roofs and walls. See
//! `docs/formats/ground_boxes.md`.

use super::{PodError, TextureCell};

/// One box per terrain cell at most. Cell `(column, row)` covers the same ground as the
/// texture map's cell of that number, from sample `(column, row)` to sample
/// `(column + 1, row + 1)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroundBoxes {
    size: usize,
    lower: Vec<u8>,
    upper: Vec<u8>,
    /// Six per cell, as written. Empty when the face file is missing or the wrong size.
    faces: Vec<u16>,
}

/// One box, from its bottom to its top, with a texture on each face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundBox {
    /// Height of the bottom, in heightmap steps.
    pub lower: u8,
    /// Height of the top, in heightmap steps. Never 0: that means no box.
    pub upper: u8,
    /// A texture for each face, in the order of `face`, in the same code as the texture
    /// map. `None` for a track without a face file.
    pub faces: Option<[TextureCell; 6]>,
}

/// Which face of a box each of `GroundBox::faces` is, in MTM2's axes.
pub mod face {
    /// The side facing -Z.
    pub const SOUTH: usize = 0;
    /// The side facing +Z.
    pub const NORTH: usize = 1;
    /// The side facing +X.
    pub const EAST: usize = 2;
    /// The side facing -X.
    pub const WEST: usize = 3;
    pub const TOP: usize = 4;
    pub const BOTTOM: usize = 5;
}

/// Bytes of the face file for each cell: six faces of two bytes.
const FACE_BYTES: usize = 12;

impl GroundBoxes {
    /// `lower` and `upper` are the .RA0 and .RA1 files, `faces` the .CL0 file, and `size`
    /// the heightmap's, which they have to match. A face file of the wrong size is left
    /// out rather than costing the track its boxes.
    pub fn parse(
        lower: &[u8],
        upper: &[u8],
        faces: Option<&[u8]>,
        size: usize,
    ) -> Result<Self, PodError> {
        for heights in [lower, upper] {
            if heights.len() != size * size {
                return Err(PodError::BadSize {
                    expected: format!(
                        "{} bytes (one per cell of a {size} x {size} map)",
                        size * size
                    ),
                    actual: heights.len(),
                });
            }
        }
        let faces = match faces {
            Some(bytes) if bytes.len() == size * size * FACE_BYTES => bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| u16::from_le_bytes(pair))
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            size,
            lower: lower.to_vec(),
            upper: upper.to_vec(),
            faces,
        })
    }

    /// Number of cells along each side.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The box on a cell, if there is one. The world repeats, so indices wrap around.
    pub fn get(&self, column: usize, row: usize) -> Option<GroundBox> {
        let index = (row % self.size) * self.size + column % self.size;
        let upper = self.upper[index];
        if upper == 0 {
            return None;
        }
        let faces = (!self.faces.is_empty()).then(|| {
            std::array::from_fn(|face| TextureCell::from_bits(self.faces[index * 6 + face]))
        });
        Some(GroundBox {
            lower: self.lower[index],
            upper,
            faces,
        })
    }

    /// Every box, with the cell it stands on, row by row.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, GroundBox)> + '_ {
        (0..self.size * self.size).filter_map(|index| {
            let (column, row) = (index % self.size, index / self.size);
            self.get(column, row).map(|found| (column, row, found))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_with_a_top_has_a_box() {
        // Two by two. Only cell (1, 0) has a top.
        let boxes = GroundBoxes::parse(&[0, 5, 9, 0], &[0, 7, 0, 0], None, 2).unwrap();
        assert_eq!(boxes.get(0, 0), None);
        // The bottom file has a value where there is no top, which counts for nothing.
        assert_eq!(boxes.get(0, 1), None);
        assert_eq!(
            boxes.get(1, 0),
            Some(GroundBox {
                lower: 5,
                upper: 7,
                faces: None
            })
        );
        assert_eq!(boxes.get(3, 2), boxes.get(1, 0));
        assert_eq!(
            boxes.iter().map(|(c, r, _)| (c, r)).collect::<Vec<_>>(),
            [(1, 0)]
        );
    }

    #[test]
    fn faces_are_read_in_the_texture_maps_code() {
        let mut faces = vec![0u8; FACE_BYTES];
        // The top: texture 0x08e, turned three times.
        faces[face::TOP * 2..face::TOP * 2 + 2].copy_from_slice(&0xc08eu16.to_le_bytes());
        let boxes = GroundBoxes::parse(&[58], &[60], Some(&faces), 1).unwrap();
        let faces = boxes.get(0, 0).unwrap().faces.unwrap();
        assert_eq!(faces[face::TOP].texture, 0x8e);
        assert_eq!(faces[face::TOP].rotation, 3);
        assert_eq!(faces[face::BOTTOM].texture, 0);
    }

    #[test]
    fn a_face_file_of_the_wrong_size_is_left_out() {
        let boxes = GroundBoxes::parse(&[1], &[2], Some(&[0; 5]), 1).unwrap();
        assert_eq!(boxes.get(0, 0).unwrap().faces, None);
    }

    #[test]
    fn heights_of_the_wrong_size_are_an_error() {
        let error = GroundBoxes::parse(&[1, 2], &[2], None, 1).unwrap_err();
        assert!(matches!(error, PodError::BadSize { actual: 2, .. }));
    }
}
