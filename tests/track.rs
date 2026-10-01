use bevy::prelude::*;
use monster_truck_rural_ruckus::track::{Diagonals, HeightGrid, build_collider, builtin_track};

/// Guards the heightfield memory layout and triangle split: a transposed or mirrored
/// collider would still look plausible in game, but the truck would drive on terrain
/// that isn't drawn there, and the wrong diagonal would put bumps where the mesh and
/// `height_at` have dips.
#[test]
fn collider_matches_height_grid() {
    assert_collider_matches(&builtin_track().heights);
}

/// The same, for terrain whose cells alternate their diagonal, as Monster Truck Madness 2
/// tracks do. Bumpy on purpose: on a smooth slope both diagonals give the same answer.
#[test]
fn collider_matches_a_checkerboard_height_grid() {
    let bumpy = HeightGrid::from_vertex_fn(257, 512.0, |col, row| {
        ((col * 7 + row * 13) % 11) as f32 + ((col * row) % 5) as f32
    });
    assert_collider_matches(&bumpy.with_diagonals(Diagonals::Checkerboard));
}

fn assert_collider_matches(grid: &HeightGrid) {
    let collider = build_collider(grid);

    // Asymmetric sample points: on grid vertices, then inside each triangle of a cell.
    for (col, row) in [(30, 200), (200, 30), (128, 100), (77, 191), (78, 191)] {
        for (dx, dz) in [
            (0.0, 0.0),
            (0.3, 0.4),
            (0.8, 0.7),
            (0.5, 0.5),
            (0.8, 0.1),
            (0.1, 0.8),
        ] {
            let (x, z) = (
                grid.coord(col) + dx * grid.cell(),
                grid.coord(row) + dz * grid.cell(),
            );
            let (distance, _) = collider
                .cast_ray(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::new(x, 100.0, z),
                    Vec3::NEG_Y,
                    200.0,
                    true,
                )
                .expect("ray should hit the terrain");
            let hit = 100.0 - distance;
            let expected = grid.height_at(x, z);
            assert!(
                (hit - expected).abs() < 1e-3,
                "at ({x}, {z}): collider {hit} vs height_at {expected}",
            );
        }
    }
}
