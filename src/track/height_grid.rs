//! The shape of the ground: a square grid of heights, centred on the world origin.
//!
//! The render mesh, the Rapier heightfield and `height_at` all split each cell into
//! the same two triangles, so they agree between vertices as well as on them. Which two
//! is up to the grid's `Diagonals`.

use bevy::math::Vec3;

/// World-space coordinate of grid line `index` along either horizontal axis.
pub fn grid_coord(index: usize, resolution: usize, size: f32) -> f32 {
    (index as f32 / (resolution - 1) as f32 - 0.5) * size
}

/// How the square cells of a grid are split into triangles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Diagonals {
    /// Every cell is split from its +X corner to its +Z corner.
    #[default]
    Uniform,
    /// Cells alternate like a chessboard: those whose column plus row is even are split
    /// from their origin corner to the far one instead. Monster Truck Madness 2 terrain
    /// is built this way.
    Checkerboard,
}

#[derive(Clone, Debug)]
pub struct HeightGrid {
    /// Number of vertices along each side.
    resolution: usize,
    /// Side length in metres.
    size: f32,
    /// Heights in metres, row by row: rows run along Z and columns along X.
    heights: Vec<f32>,
    diagonals: Diagonals,
}

impl HeightGrid {
    /// Samples `height(x, z)` at every vertex, row by row.
    pub fn from_fn(resolution: usize, size: f32, mut height: impl FnMut(f32, f32) -> f32) -> Self {
        Self::from_vertex_fn(resolution, size, |col, row| {
            height(
                grid_coord(col, resolution, size),
                grid_coord(row, resolution, size),
            )
        })
    }

    /// Asks `height(col, row)` for every vertex, row by row.
    pub fn from_vertex_fn(
        resolution: usize,
        size: f32,
        mut height: impl FnMut(usize, usize) -> f32,
    ) -> Self {
        assert!(resolution >= 2, "a height grid needs at least one cell");
        let mut heights = Vec::with_capacity(resolution * resolution);
        for row in 0..resolution {
            for col in 0..resolution {
                heights.push(height(col, row));
            }
        }
        Self {
            resolution,
            size,
            heights,
            diagonals: Diagonals::default(),
        }
    }

    pub fn with_diagonals(mut self, diagonals: Diagonals) -> Self {
        self.diagonals = diagonals;
        self
    }

    /// Which way the ground faces at a world-space position, not on the flat triangles
    /// that are drawn and driven on, but on a smooth surface (a Catmull-Rom spline) through
    /// every vertex of the grid: for lighting the ground as if it were rounded off.
    /// Positions outside the grid are taken to be on its nearest edge.
    ///
    /// On a vertex it is the normal `track/mesh.rs` gives that vertex, from the heights of
    /// its neighbours, and it turns smoothly from there to the next, with no crease where
    /// two triangles meet. An even slope keeps its own normal. A crease sharper than
    /// `SHARP_CREASE`, such as the lip of a ramp over a cliff, is kept: the surface on each
    /// side of it goes on at that side's own slope.
    pub fn smooth_normal(&self, x: f32, z: f32) -> Vec3 {
        let (col, t_col) = self.to_grid(x);
        let (row, t_row) = self.to_grid(z);
        let last = (self.resolution - 1) as isize;
        let vertex = |col: isize, row: isize| {
            self.vertex(col.clamp(0, last) as usize, row.clamp(0, last) as usize)
        };
        let cell = self.cell();

        // Along X within each of the four nearest rows: the height, and its slope.
        let mut heights = [0.0; 4];
        let mut slopes_x = [0.0; 4];
        for (index, dz) in [-1, 0, 1, 2].into_iter().enumerate() {
            let samples = [-1, 0, 1, 2].map(|dx| vertex(col as isize + dx, row as isize + dz));
            let samples = keep_creases(samples, sharp_creases(samples, cell));
            heights[index] = catmull_rom(samples, t_col);
            slopes_x[index] = catmull_rom_slope(samples, t_col) / cell;
        }
        // Then along Z between them. The slope along X goes the same way, with the same
        // creases kept, since the height is made from it by the same sum.
        let creases = sharp_creases(heights, cell);
        let slope_x = catmull_rom(keep_creases(slopes_x, creases), t_row);
        let slope_z = catmull_rom_slope(keep_creases(heights, creases), t_row) / cell;
        Vec3::new(-slope_x, 1.0, -slope_z).normalize()
    }

    /// Whether the cell whose lowest corner is vertex (`col`, `row`) is split from that
    /// corner to the opposite one, rather than from its +X corner to its +Z corner.
    pub fn splits_from_origin(&self, col: usize, row: usize) -> bool {
        self.diagonals == Diagonals::Checkerboard && (col + row).is_multiple_of(2)
    }

    pub fn resolution(&self) -> usize {
        self.resolution
    }

    pub fn size(&self) -> f32 {
        self.size
    }

    /// Side length of one cell, in metres.
    pub fn cell(&self) -> f32 {
        self.size / (self.resolution - 1) as f32
    }

    /// World-space coordinate of grid line `index` along either horizontal axis.
    pub fn coord(&self, index: usize) -> f32 {
        grid_coord(index, self.resolution, self.size)
    }

    /// Height of the vertex at column `col` (along X) and row `row` (along Z).
    pub fn vertex(&self, col: usize, row: usize) -> f32 {
        self.heights[row * self.resolution + col]
    }

    /// Ground height at a world-space position. Positions outside the grid get the
    /// height of the nearest edge.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let (col, fx) = self.to_grid(x);
        let (row, fz) = self.to_grid(z);

        let h00 = self.vertex(col, row);
        let h10 = self.vertex(col + 1, row); // +X
        let h01 = self.vertex(col, row + 1); // +Z
        let h11 = self.vertex(col + 1, row + 1);

        // Interpolate within whichever of the cell's two triangles holds the point.
        if self.splits_from_origin(col, row) {
            if fx >= fz {
                h00 + fx * (h10 - h00) + fz * (h11 - h10)
            } else {
                h00 + fz * (h01 - h00) + fx * (h11 - h01)
            }
        } else if fx + fz <= 1.0 {
            h00 + fx * (h10 - h00) + fz * (h01 - h00)
        } else {
            h11 + (1.0 - fx) * (h01 - h11) + (1.0 - fz) * (h10 - h11)
        }
    }

    /// The cell a world-space coordinate along either axis is in, and how far across it,
    /// from 0 to 1. Beyond the grid, the nearest edge.
    fn to_grid(&self, world: f32) -> (usize, f32) {
        let cells = ((world / self.size + 0.5) * (self.resolution - 1) as f32)
            .clamp(0.0, (self.resolution - 1) as f32);
        let index = (cells as usize).min(self.resolution - 2);
        (index, cells - index as f32)
    }
}

/// How much the slope may change at a vertex, as rise over run, before the smooth surface
/// keeps the crease there sharp. 1 is from level ground to 45 degrees. The creases of a
/// road change by much less than that. A ramp's lip over a cliff changes by far more:
/// Alpine's after checkpoint 8 changes by 10. Make it smaller and more of the ground
/// keeps its sharp creases. Make it larger and the steep sides of more creases bend the
/// ground near them: rounded off, a ramp that ends at a cliff looks as if it levels off
/// before its lip, and a road at the foot of a cliff as if it dipped.
const SHARP_CREASE: f32 = 1.0;

/// Whether the crease at the second and at the third of four evenly spaced heights, `cell`
/// metres apart, is sharper than `SHARP_CREASE`.
fn sharp_creases(p: [f32; 4], cell: f32) -> [bool; 2] {
    let sharp = |before: f32, at: f32, after: f32| {
        ((after - at) - (at - before)).abs() > SHARP_CREASE * cell
    };
    [sharp(p[0], p[1], p[2]), sharp(p[1], p[2], p[3])]
}

/// The four values with the outer one past each sharp crease replaced, so that a curve
/// through them goes on at the slope of its own side, as if the ground past the crease
/// continued in that way.
fn keep_creases(mut p: [f32; 4], [first, second]: [bool; 2]) -> [f32; 4] {
    if first {
        p[0] = 2.0 * p[1] - p[2];
    }
    if second {
        p[3] = 2.0 * p[2] - p[1];
    }
    p
}

/// The point a fraction `t` of the way from `p[1]` to `p[2]` on the smooth curve through
/// all four, which are evenly spaced.
fn catmull_rom(p: [f32; 4], t: f32) -> f32 {
    0.5 * (2.0 * p[1]
        + (p[2] - p[0]) * t
        + (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]) * t * t
        + (3.0 * p[1] - p[0] - 3.0 * p[2] + p[3]) * t * t * t)
}

/// How fast `catmull_rom` changes with `t`: its slope per spacing of the four values.
fn catmull_rom_slope(p: [f32; 4], t: f32) -> f32 {
    0.5 * ((p[2] - p[0])
        + 2.0 * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]) * t
        + 3.0 * (3.0 * p[1] - p[0] - 3.0 * p[2] + p[3]) * t * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3x3 vertices over 4 m, so 2 m cells spanning -2..2. Not a plane, so the two
    /// triangles of a cell give different answers.
    fn grid() -> HeightGrid {
        HeightGrid::from_fn(3, 4.0, |x, z| x * x + 10.0 * z)
    }

    #[test]
    fn exact_on_vertices() {
        let grid = grid();
        for row in 0..3 {
            for col in 0..3 {
                let (x, z) = (grid.coord(col), grid.coord(row));
                assert_eq!(grid.height_at(x, z), x * x + 10.0 * z);
                assert_eq!(grid.vertex(col, row), x * x + 10.0 * z);
            }
        }
    }

    #[test]
    fn interpolates_within_each_triangle() {
        let grid = grid();
        // The cell from (0, 0) to (2, 2) has corner heights 0, 4 (+X), 20 (+Z) and 24.
        // Near the origin corner: 0 + 0.25 * 4 + 0.25 * 20.
        assert!((grid.height_at(0.5, 0.5) - 6.0).abs() < 1e-5);
        // Near the far corner: 24 + 0.25 * (20 - 24) + 0.25 * (4 - 24).
        assert!((grid.height_at(1.5, 1.5) - 18.0).abs() < 1e-5);
        // On the shared diagonal both triangles agree: halfway between 4 and 20.
        assert!((grid.height_at(1.0, 1.0) - 12.0).abs() < 1e-5);
    }

    #[test]
    fn checkerboard_cells_alternate_their_diagonal() {
        let grid = grid().with_diagonals(Diagonals::Checkerboard);
        // The cell from (0, 0) to (2, 2) is column 1, row 1: even, so split from its
        // origin corner (height 0) to the far one (24). Its corners are 0, 4, 20 and 24.
        assert!(grid.splits_from_origin(1, 1));
        assert!((grid.height_at(1.0, 1.0) - 12.0).abs() < 1e-5);
        // Towards +X of that diagonal: 0 + 0.75 * 4 + 0.25 * (24 - 4).
        assert!((grid.height_at(1.5, 0.5) - 8.0).abs() < 1e-5);
        // Towards +Z of it: 0 + 0.75 * 20 + 0.25 * (24 - 20).
        assert!((grid.height_at(0.5, 1.5) - 16.0).abs() < 1e-5);

        // Its neighbour, column 0, row 1, is odd and keeps the usual split.
        assert!(!grid.splits_from_origin(0, 1));
        assert_eq!(grid.height_at(-1.5, 0.5), self::grid().height_at(-1.5, 0.5));
        // Vertices are unaffected.
        assert_eq!(grid.height_at(2.0, 0.0), 4.0);
    }

    /// Normals that agree to within a hundredth of a degree or so.
    fn assert_close(a: Vec3, b: Vec3, at: &str) {
        assert_within(a, b, 2e-4, at);
    }

    fn assert_within(a: Vec3, b: Vec3, tolerance: f32, at: &str) {
        assert!(a.distance(b) < tolerance, "{at}: {a} vs {b}");
    }

    /// Rolling ground with no crease sharp enough to keep.
    fn rolling() -> HeightGrid {
        HeightGrid::from_fn(9, 80.0, |x, z| {
            3.0 * (0.1 * x).sin() + 2.0 * (0.07 * z).cos()
        })
    }

    #[test]
    fn an_even_slope_keeps_its_own_normal() {
        let slope = HeightGrid::from_fn(6, 50.0, |x, z| 0.3 * x - 0.2 * z + 4.0);
        let normal = Vec3::new(-0.3, 1.0, 0.2).normalize();
        // Away from the edges, where there is nothing to continue it.
        for (x, z) in [(-7.5, 3.0), (1.0, -12.0), (11.0, 9.5), (5.0, 5.0)] {
            assert_close(slope.smooth_normal(x, z), normal, &format!("({x}, {z})"));
        }
    }

    #[test]
    fn on_a_vertex_it_is_the_normal_from_the_neighbours() {
        let grid = rolling();
        for (col, row) in [(2, 3), (4, 4), (6, 2)] {
            let (x, z) = (grid.coord(col), grid.coord(row));
            let slope_x =
                (grid.vertex(col + 1, row) - grid.vertex(col - 1, row)) / (2.0 * grid.cell());
            let slope_z =
                (grid.vertex(col, row + 1) - grid.vertex(col, row - 1)) / (2.0 * grid.cell());
            let from_neighbours = Vec3::new(-slope_x, 1.0, -slope_z).normalize();
            assert_close(
                grid.smooth_normal(x, z),
                from_neighbours,
                &format!("vertex ({col}, {row})"),
            );
        }
    }

    #[test]
    fn it_turns_smoothly_across_the_edges_of_cells() {
        let grid = rolling();
        // Where the flat triangles on either side slope differently.
        let edge = grid.coord(3);
        let step = 1e-3;
        for across in [-15.0, -2.5, 7.0] {
            // Across an edge along Z, and across one along X.
            assert_close(
                grid.smooth_normal(edge - step, across),
                grid.smooth_normal(edge + step, across),
                "X",
            );
            assert_close(
                grid.smooth_normal(across, edge - step),
                grid.smooth_normal(across, edge + step),
                "Z",
            );
        }
        // Whereas the flat triangles have a crease there.
        let flat = |x: f32| grid.height_at(x, 3.0);
        let (before, after) = (flat(edge) - flat(edge - 1.0), flat(edge + 1.0) - flat(edge));
        assert!((before - after).abs() > 0.01);
    }

    #[test]
    fn level_ground_beside_a_cliff_stays_level() {
        // Flat at 0, then a 30 m wall between columns 3 and 4, then flat at 30.
        let cliff = HeightGrid::from_vertex_fn(8, 70.0, |col, _| if col >= 4 { 30.0 } else { 0.0 });
        // The whole cell before the wall's, up to its edge, and the whole cell after.
        for x in [-15.0, -10.0, -5.1, 5.1, 10.0, 15.0] {
            assert_close(cliff.smooth_normal(x, 0.0), Vec3::Y, &format!("x = {x}"));
        }
    }

    #[test]
    fn a_ramp_that_ends_at_a_cliff_keeps_its_slope_to_the_lip() {
        // Level, then a ramp 2.5 m up over one 10 m cell to its lip, then a 95 m drop:
        // Alpine's after checkpoint 8.
        let heights = [150.0, 150.0, 150.0, 152.5, 55.0, 55.0, 55.0, 55.0];
        let ramp = HeightGrid::from_vertex_fn(8, 70.0, |col, _| heights[col]);
        let ramp_normal = Vec3::new(-0.25, 1.0, 0.0).normalize();
        // Just short of the lip it faces as the ramp does, not bent over by the cliff.
        let lip = ramp.smooth_normal(ramp.coord(3) - 0.01, 0.0);
        assert_within(lip, ramp_normal, 1e-3, "lip");
        // At its foot, a gentle crease, it is rounded off: part way between level and ramp.
        let foot = ramp.smooth_normal(ramp.coord(2) + 1.0, 0.0);
        assert!(foot.x < 0.0 && foot.x > ramp_normal.x, "{foot}");
    }

    #[test]
    fn clamps_outside_the_grid() {
        let grid = grid();
        assert_eq!(grid.height_at(-50.0, 0.0), grid.height_at(-2.0, 0.0));
        assert_eq!(grid.height_at(50.0, 50.0), grid.vertex(2, 2));
    }
}
