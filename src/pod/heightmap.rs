//! The terrain heightmap (.RAW named by the level file). See `docs/formats/terrain.md`.

use super::PodError;

/// Side length of one terrain cell, in feet.
pub const FEET_PER_CELL: f32 = 32.0;
/// Height of one heightmap step, in feet.
pub const FEET_PER_HEIGHT_STEP: f32 = 2.0;

/// A square grid of heights, one byte each. Rows run along MTM2's Z axis starting at
/// z = 0, and columns along X starting at x = 0. Sample `(column, row)` is the height
/// at `(column * FEET_PER_CELL, row * FEET_PER_CELL)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heightmap {
    size: usize,
    steps: Vec<u8>,
}

impl Heightmap {
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
            steps: bytes.to_vec(),
        })
    }

    /// Number of samples along each side.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Height in raw steps. The world repeats, so indices wrap around.
    pub fn steps(&self, column: usize, row: usize) -> u8 {
        self.steps[(row % self.size) * self.size + column % self.size]
    }

    /// Height in feet. The world repeats, so indices wrap around.
    pub fn feet(&self, column: usize, row: usize) -> f32 {
        self.steps(column, row) as f32 * FEET_PER_HEIGHT_STEP
    }

    /// Whether the cell between samples (`column`, `row`) and (`column + 1`, `row + 1`)
    /// is split into triangles from the first of those corners to the second. The other
    /// cells are split across, from (`column + 1`, `row`) to (`column`, `row + 1`).
    /// Cells alternate like a chessboard.
    pub fn splits_from_origin(column: usize, row: usize) -> bool {
        (column + row).is_multiple_of(2)
    }

    /// Height of the ground in feet at a position in feet, on the triangles the terrain
    /// is made of.
    pub fn ground_feet(&self, x: f32, z: f32) -> f32 {
        let world = self.size as f32 * FEET_PER_CELL;
        let (x, z) = (
            x.rem_euclid(world) / FEET_PER_CELL,
            z.rem_euclid(world) / FEET_PER_CELL,
        );
        let (column, row) = (x as usize, z as usize);
        let (u, v) = (x - column as f32, z - row as f32);

        let h00 = self.feet(column, row);
        let h10 = self.feet(column + 1, row);
        let h01 = self.feet(column, row + 1);
        let h11 = self.feet(column + 1, row + 1);

        if Self::splits_from_origin(column, row) {
            if u >= v {
                h00 + u * (h10 - h00) + v * (h11 - h10)
            } else {
                h00 + v * (h01 - h00) + u * (h11 - h01)
            }
        } else if u + v <= 1.0 {
            h00 + u * (h10 - h00) + v * (h01 - h00)
        } else {
            h11 + (1.0 - u) * (h01 - h11) + (1.0 - v) * (h10 - h11)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows_along_z_and_wraps() {
        let heightmap = Heightmap::parse(&[0, 1, 2, 10, 11, 12, 20, 21, 22]).unwrap();
        assert_eq!(heightmap.size(), 3);
        assert_eq!(heightmap.steps(2, 0), 2);
        assert_eq!(heightmap.steps(0, 2), 20);
        assert_eq!(heightmap.steps(3, 4), 10);
        assert_eq!(heightmap.feet(1, 1), 22.0);
    }

    #[test]
    fn ground_follows_the_chessboard_of_triangles() {
        // Two by two, wrapping. Steps 0, 5 / 10, 25, so feet 0, 10 / 20, 50.
        let heightmap = Heightmap::parse(&[0, 5, 10, 25]).unwrap();
        assert_eq!(heightmap.ground_feet(0.0, 0.0), 0.0);
        assert_eq!(heightmap.ground_feet(32.0, 32.0), 50.0);

        // Cell (0, 0) is even: split from its origin corner to the far one, so the
        // middle is halfway between 0 and 50.
        assert_eq!(heightmap.ground_feet(16.0, 16.0), 25.0);
        assert_eq!(heightmap.ground_feet(24.0, 8.0), 0.75 * 10.0 + 0.25 * 40.0);

        // Cell (1, 0) is odd: corners 10, 0 (wrapped) / 50, 20, split across, so the
        // middle is halfway between its +X and +Z corners, 0 and 50.
        assert_eq!(heightmap.ground_feet(48.0, 16.0), 25.0);
        assert_eq!(
            heightmap.ground_feet(40.0, 8.0),
            10.0 + 0.25 * -10.0 + 0.25 * 40.0
        );
    }

    #[test]
    fn rejects_sizes_that_are_not_square() {
        assert!(matches!(
            Heightmap::parse(&[0; 10]),
            Err(PodError::BadSize { .. })
        ));
        assert!(matches!(
            Heightmap::parse(&[]),
            Err(PodError::BadSize { .. })
        ));
    }
}
