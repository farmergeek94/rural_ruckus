//! Converts real Monster Truck Madness 2 tracks. Such tracks can't be committed, so this
//! reads `tracks/` and passes trivially when a track isn't there.

use std::path::Path;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use monster_truck_rural_ruckus::base_game::BaseGame;
use monster_truck_rural_ruckus::pod;
use monster_truck_rural_ruckus::scenery::SceneryPlugin;
use monster_truck_rural_ruckus::track::{
    Footing, Track, TrackData, TrackPlugin, TrackSettings, load_pod,
};
use monster_truck_rural_ruckus::truck::TireContacts;

/// The archives in a folder of the repository and in every folder under it, leaving out
/// anything else there (the `.gitkeep` that keeps an empty folder in git).
fn pods(folder: &str) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join(folder)];
    while let Some(folder) = folders.pop() {
        for path in std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|file| file.path())
        {
            if path.is_dir() {
                folders.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pod"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn my_track() -> Option<TrackData> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/MyTrack.pod");
    path.exists()
        .then(|| load_pod(&path, &BaseGame::none()).unwrap())
}

#[test]
fn converts_to_the_games_conventions() {
    let Some(track) = my_track() else { return };
    assert_eq!(track.name, "My First Track");

    // 256 cells of 32 ft, centred on the origin.
    let feet = 0.3048;
    assert!((track.heights.size() - 8192.0 * feet).abs() < 0.01);
    assert_eq!(track.heights.resolution(), 257);

    // Pole position is at (4338.28, 3887.13) ft, east and south of the middle. South is
    // +Z here, and the truck faces north, towards the finish line 77 ft ahead.
    let start = track.start.position;
    assert!(
        start.distance(Vec2::new(242.28, 208.87) * feet) < 0.01,
        "{start}"
    );
    assert!(track.start.yaw.sin().abs() < 0.05 && track.start.yaw.cos() > 0.99);
    // The grid is on flat ground 120 ft up, where the file puts the course.
    let ground = track.heights.height_at(start.x, start.y);
    assert!(
        (ground - 120.0 * feet).abs() < 0.01,
        "start is {ground} m up"
    );

    // The finish line is listed last in the file and comes first here.
    assert_eq!(track.gates.len(), 2);
    let finish = &track.gates[0];
    let to_finish = finish.center - start;
    assert!(
        (to_finish.length() - 77.0 * feet).abs() < 2.0,
        "{to_finish}"
    );
    assert!(to_finish.normalize().dot(finish.direction()) > 0.95);

    // The far checkpoint is on the west side of the circuit, driven southwards.
    assert!(track.gates[1].center.x < finish.center.x - 100.0);
    assert!(track.gates[1].direction().dot(Vec2::Y) > 0.99);

    // The course shows up in the surface tint: under the start, but not out in the grass.
    let resolution = track.heights.resolution();
    let tint_at = |position: Vec2| {
        let index = |world: f32| {
            ((world / track.heights.size() + 0.5) * (resolution - 1) as f32).round() as usize
        };
        track.surface[index(position.y) * resolution + index(position.x)]
    };
    assert!(tint_at(start) > 0.4, "start tint {}", tint_at(start));
    assert_eq!(tint_at(start + Vec2::new(60.0, 0.0)), 0.0);

    // Its textures belong to the base game, so there are none to show.
    assert!(track.ground.is_none());
}

/// The whole starting grid is converted, not pole position alone: every place clear of
/// the others and facing much the way pole position does. Not always behind it: My First
/// Track's grid is four rows of two behind pole position, and Alpine's second row of four
/// stands in front of its first. Not always straight either: The Tight Corners' rows of
/// two follow a bend, turning 14 degrees a row, to 44 degrees at the back.
#[test]
fn every_real_track_has_a_whole_starting_grid() {
    use monster_truck_rural_ruckus::track::yaw_direction;
    for file in pods("tracks") {
        let track = load_pod(&file, &BaseGame::none()).unwrap();
        let name = &track.name;
        assert!(!track.grid.is_empty(), "{name}");
        let ahead = yaw_direction(track.start.yaw);
        let places: Vec<_> = (0..=track.grid.len())
            .map(|place| track.grid_place(place))
            .collect();
        for (index, place) in places.iter().enumerate() {
            println!("{name}: place {index} at {}", place.position);
            assert!(yaw_direction(place.yaw).dot(ahead) > 0.5, "{name} {index}");
            for other in &places[..index] {
                let apart = other.position.distance(place.position);
                assert!(
                    apart > 4.0,
                    "{name}: place {index} is {apart} m from another"
                );
            }
        }
    }
}

/// A list of tracks reads each archive's name alone, which must be the name that loading
/// the whole track gives, and must tell a track from a truck.
#[test]
fn peeking_gives_the_name_that_loading_does() {
    use monster_truck_rural_ruckus::track::peek_pod;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in pods("tracks") {
        let name = peek_pod(&file).unwrap().expect("a track in tracks/");
        assert_eq!(name, load_pod(&file, &BaseGame::none()).unwrap().name);
    }
    for file in pods("trucks") {
        assert_eq!(peek_pod(&file), Ok(None));
    }
    assert!(peek_pod(&root.join("tracks/no_such_track.pod")).is_err());
}

/// The importer flips Z, which turns the terrain's chessboard of triangles over. If it
/// got that wrong, the game's ground would differ from the track's own by feet wherever
/// a cell isn't flat. Alpine Mountains is steep nearly everywhere.
#[test]
fn converted_ground_matches_the_tracks_own() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/AlpineMtns.pod");
    if !path.exists() {
        return;
    }
    let archive = pod::PodArchive::parse(std::fs::read(&path).unwrap()).unwrap();
    let original = pod::Track::from_archive(&archive).unwrap();
    let converted = load_pod(&path, &BaseGame::none()).unwrap();
    assert_eq!(converted.gates.len(), 10);

    // It carries its own ground textures: 43 of the 100 it lists are used on the ground,
    // and its ground boxes use 22 more.
    let ground = converted.ground.as_ref().expect("ground textures");
    assert_eq!((ground.tile_size, ground.tiles.len()), (64, 43 + 22));
    assert_eq!(ground.cells.iter().map(|cell| cell.tile).max(), Some(42));
    assert_eq!(ground.cells.len(), 256 * 256);
    assert!(ground.tiles.iter().all(|tile| tile.len() == 64 * 64 * 4));

    let feet = 0.3048;
    let mut uneven = 0;
    for scenery in &original.situation.boxes {
        let [x, _, z] = scenery.position;
        let expected = original.heightmap.ground_feet(x, z) * feet;
        let ours = converted
            .heights
            .height_at((x - 4096.0) * feet, (4096.0 - z) * feet);
        assert!(
            (ours - expected).abs() < 0.002,
            "at ({x}, {z}) ft: {ours} m vs {expected} m"
        );

        // Count the places where the choice of diagonal matters, to be sure this test
        // could fail: the two ways of splitting the cell differ by over 10 cm.
        let (column, row) = ((x / 32.0) as usize, (z / 32.0) as usize);
        let corner = |dc, dr| original.heightmap.feet(column + dc, row + dr);
        if ((corner(0, 0) + corner(1, 1)) - (corner(1, 0) + corner(0, 1))).abs() > 1.0 {
            uneven += 1;
        }
    }
    assert!(uneven > 50, "only {uneven} placements on uneven cells");
}

/// Paints a map of part of a track from the converted ground textures, to look at. It
/// samples them exactly as the terrain mesh does: each cell's corners give the texture
/// coordinates, interpolated across the cell. Writes a PPM image to the path in the
/// `GROUND_MAP` environment variable, and does nothing without it:
/// `GROUND_MAP=/tmp/map.ppm cargo test --test pod_track paints -- --ignored`. The track is
/// Alpine, or the archive in `tracks/` that `GROUND_MAP_TRACK` names.
#[test]
#[ignore = "writes an image for a person to look at"]
fn paints_a_map_of_the_ground() {
    let Ok(output) = std::env::var("GROUND_MAP") else {
        return;
    };
    let file = std::env::var("GROUND_MAP_TRACK").unwrap_or_else(|_| "AlpineMtns.pod".into());
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tracks")
        .join(&file);
    let track = load_pod(&path, &BaseGame::none()).unwrap();
    let ground = track
        .ground
        .as_ref()
        .expect("the track carries its textures");

    // A window of cells around the start, at 16 pixels per cell. North (-Z) is up.
    let cells_per_side = track.heights.resolution() - 1;
    let to_cell =
        |world: f32| ((world / track.heights.size() + 0.5) * cells_per_side as f32) as usize;
    let (span, scale) = (48, 16);
    let first_col = to_cell(track.start.position.x).saturating_sub(span / 2);
    let first_row = to_cell(track.start.position.y).saturating_sub(span / 2);

    let side = span * scale;
    let mut pixels = Vec::with_capacity(side * side * 3);
    for y in 0..side {
        for x in 0..side {
            let cell =
                &ground.cells[(first_row + y / scale) * cells_per_side + first_col + x / scale];
            let (u, v) = (
                (x % scale) as f32 / scale as f32 + 0.5 / scale as f32,
                (y % scale) as f32 / scale as f32 + 0.5 / scale as f32,
            );
            let [c00, c10, c01, c11] = cell.corners.map(Vec2::from);
            let coords = c00.lerp(c10, u).lerp(c01.lerp(c11, u), v);
            let pixel = (coords * ground.tile_size as f32)
                .floor()
                .clamp(Vec2::ZERO, Vec2::splat(ground.tile_size as f32 - 1.0));
            let index = (pixel.y as usize * ground.tile_size + pixel.x as usize) * 4;
            pixels.extend_from_slice(&ground.tiles[cell.tile][index..index + 3]);
        }
    }

    let mut image = format!("P6 {side} {side} 255\n").into_bytes();
    image.extend(pixels);
    std::fs::write(output, image).unwrap();
}

/// The map the front end shows of a track, to look at:
/// `TRACK_MAP=/tmp/map.ppm cargo test --test pod_track paints_the_track_map -- --ignored`
#[test]
#[ignore = "writes an image for a person to look at"]
fn paints_the_track_map() {
    let Ok(output) = std::env::var("TRACK_MAP") else {
        return;
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/AlpineMtns.pod");
    let started = std::time::Instant::now();
    let track = load_pod(&path, &BaseGame::none()).unwrap();
    let loaded = started.elapsed();
    let side = 320;
    let map = monster_truck_rural_ruckus::track::map_image(&track, side);
    println!(
        "loaded in {loaded:?}, painted in {:?}",
        started.elapsed() - loaded
    );

    let mut image = format!("P6 {side} {side} 255\n").into_bytes();
    image.extend(map.as_chunks::<4>().0.iter().flat_map(|pixel| &pixel[..3]));
    std::fs::write(output, image).unwrap();
}

/// Smooth terrain changes only how the ground is lit: the ground raced on, which the
/// collider, the mesh and everything standing on it are made from, is the track's own.
#[test]
fn smooth_terrain_leaves_the_ground_as_it_is() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/AlpineMtns.pod");
    if !path.exists() {
        return;
    }
    let original = load_pod(&path, &BaseGame::none()).unwrap();

    let mut app = App::new();
    app.insert_resource(Track(original.clone()))
        .insert_resource(TrackSettings {
            smooth_terrain: true,
            ..default()
        })
        .add_plugins((MinimalPlugins, AssetPlugin::default(), TrackPlugin))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>();
    // Into the race, so that the ground is built.
    app.update();
    let raced = &app.world().resource::<Track>().heights;

    assert_eq!(raced.resolution(), original.heights.resolution());
    for row in (0..257).step_by(8) {
        for col in (0..257).step_by(8) {
            assert_eq!(raced.vertex(col, row), original.heights.vertex(col, row));
        }
    }
}

fn alpine() -> Option<TrackData> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/AlpineMtns.pod");
    path.exists()
        .then(|| load_pod(&path, &BaseGame::none()).unwrap())
}

#[test]
fn scenery_is_converted_and_stands_on_the_ground() {
    let Some(track) = alpine() else { return };
    let scenery = &track.scenery;
    let feet = 0.3048;

    // 323 boxes in 31 models, less the finish line's invisible trigger box.
    assert_eq!((scenery.models.len(), scenery.objects.len()), (30, 322));
    assert!(scenery.models.iter().all(|model| model.name != "CKBOX.BIN"));
    assert!(scenery.tiles.iter().all(|tile| tile.len() == 64 * 64 * 4));
    for model in &scenery.models {
        assert!(!model.indices.is_empty(), "{}", model.name);
        assert_eq!(model.positions.len(), model.tiles.len());
        assert!(
            model
                .indices
                .iter()
                .all(|&index| (index as usize) < model.positions.len())
        );
        assert!(
            model
                .tiles
                .iter()
                .all(|&tile| (tile as usize) < scenery.tiles.len())
        );
        assert!(
            model
                .uvs
                .iter()
                .flatten()
                .all(|along| (0.0..=1.0).contains(along))
        );
    }

    // Guard rails stand with their lowest point on the ground, and are solid. Banners are
    // checkpoints, to be driven through.
    let number_of = |name: &str| {
        scenery
            .models
            .iter()
            .position(|model| model.name == name)
            .unwrap()
    };
    let rail = number_of("GRDRAIL2.BIN");
    let lowest = scenery.models[rail]
        .positions
        .iter()
        .map(|position| position[1])
        .fold(f32::INFINITY, f32::min);
    let rails: Vec<_> = scenery
        .objects
        .iter()
        .filter(|object| object.model == rail)
        .collect();
    assert_eq!(rails.len(), 33);
    let grounded = rails
        .iter()
        .filter(|object| (object.height_above_ground + lowest).abs() < 0.01)
        .count();
    assert!(grounded >= 30, "{grounded} of 33 rails are on the ground");
    assert!(rails.iter().all(|object| object.solid));
    let banner = number_of("PEPBAN1.BIN");
    assert!(
        scenery
            .objects
            .iter()
            .any(|object| object.model == banner && !object.solid)
    );

    // Checkpoints are as wide as their models: 110 ft banners, and a 192 ft finish line.
    assert!((track.gates[0].half_width - 96.0 * feet).abs() < 0.01);
    assert!((track.gates[1].half_width - 55.0 * feet).abs() < 0.01);

    // Faces still look outwards after the flip to our axes. In the files two thirds of
    // all faces point away from the middle of their model (the rest are insides, undersides
    // and the backs of two-sided things); inside out, it would be one third.
    let (mut outward, mut faces, mut odd_normals) = (0, 0, 0);
    for model in &scenery.models {
        let middle = model
            .positions
            .iter()
            .copied()
            .map(Vec3::from)
            .sum::<Vec3>()
            / model.positions.len() as f32;
        for triangle in model.indices.as_chunks::<3>().0 {
            let [a, b, c] =
                [0, 1, 2].map(|corner| Vec3::from(model.positions[triangle[corner] as usize]));
            let normal = (b - a).cross(c - a);
            // The normal stored with the vertex is that of the face as wound. (Roughly: a
            // few four-cornered faces in the files aren't quite flat.)
            let stored = Vec3::from(model.normals[triangle[0] as usize]);
            if normal.length() > 1e-6 && normal.normalize().dot(stored) < 0.7 {
                odd_normals += 1;
            }
            faces += 1;
            if normal.dot((a + b + c) / 3.0 - middle) > 0.0 {
                outward += 1;
            }
        }
    }
    // About one face in 25 in the files is a four-cornered one with a twist in it, whose two
    // triangles lean away from the face's own normal. More than that would be a bug.
    assert!(
        odd_normals * 10 < faces,
        "{odd_normals} of {faces} triangles disagree with their face"
    );
    assert!(
        outward * 10 > faces * 6,
        "{outward} of {faces} faces look outwards"
    );
}

/// The mipmaps setting reaches the textures that are actually made, for the ground and
/// for the scenery, not just the function that makes them.
#[test]
fn the_mipmaps_setting_reaches_the_textures() {
    use monster_truck_rural_ruckus::track::TileMaterial;

    let Some(track) = alpine() else { return };
    for (mipmaps, expected_levels) in [(true, 7), (false, 1)] {
        let mut app = App::new();
        app.insert_resource(Track(track.clone()))
            .insert_resource(TrackSettings {
                mipmaps,
                ..default()
            })
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                AssetPlugin::default(),
                RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule(),
                TrackPlugin,
                SceneryPlugin,
            ))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            // What a drawing app would have, so that the textures get made.
            .init_asset::<Image>()
            .init_asset::<TileMaterial>();
        app.update();

        let images = app.world().resource::<Assets<Image>>();
        let levels: Vec<u32> = images
            .iter()
            .map(|(_, image)| image.texture_descriptor.mip_level_count)
            .collect();
        // One array of tiles for the ground and one for the scenery.
        assert_eq!(levels, [expected_levels; 2], "with mipmaps {mipmaps}");
    }
}

/// Past checkpoint 6, Alpine's course crosses a ravine on a bridge made of ground boxes
/// (see `tests/pod_real_tracks.rs`). The race is run on the bridge, not 120 ft below it.
#[test]
fn alpine_bridge_is_solid_where_the_course_crosses_it() {
    let Some(track) = alpine() else { return };
    let feet = 0.3048;
    assert_eq!(track.ground_boxes.len(), 502);
    assert!(track.ground_boxes.iter().all(|found| found.faces.is_some()));

    let mut app = App::new();
    app.insert_resource(Track(track))
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule(),
            TrackPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>();
    app.update();

    let world = app.world_mut();
    let (collider, _) = world
        .query::<(&Collider, &Name)>()
        .iter(world)
        .find(|(_, name)| name.as_str() == "Ground boxes")
        .expect("the ground boxes have a collider");
    let track = world.resource::<Track>();

    // Along the course's line over the middle of the ravine, at (1310, 6048) ft.
    let at = Vec2::new(1310.0 - 4096.0, 4096.0 - 6048.0) * feet;
    let from = Vec3::new(at.x, 200.0, at.y);
    let hit = collider
        .cast_local_ray(from, Vec3::NEG_Y, 1000.0, true)
        .expect("the bridge is there");
    let deck = from.y - hit;
    assert!(
        (deck - 240.0 * feet).abs() < 0.01,
        "the deck is {deck} m up"
    );
    let ground = track.heights.height_at(at.x, at.y);
    assert!(
        (ground - 116.0 * feet).abs() < 0.01,
        "the ravine is {ground} m deep"
    );

    // And where the deck meets the ground, at both ends, there is no step.
    for z in [5920.0, 6176.0] {
        let end = Vec2::new(1296.0 - 4096.0, 4096.0 - z) * feet;
        let ground = track.heights.height_at(end.x, end.y);
        assert!(
            (ground - deck).abs() < 0.01,
            "at z = {z} ft the ground is {ground} m up"
        );
    }
}

/// The importer lays no triangle over another facing the same way with the texture laid on
/// differently: the two would fight for every pixel, and flicker. Baja Beach's trees, huts
/// and ship write both sides of each leaf and plank as faces of their own, and call both
/// two-sided, which once gave each side a second back. A file may hold such pairs of its
/// own accord (Rute 756 Jam's `BFVPROP.BIN` does), and those are drawn as written: what
/// is checked is that a converted model has no more of them than its file. Copies that
/// look the same (Baja's fence repeats its faces) are harmless, and not counted.
#[test]
fn scenery_is_not_drawn_twice_over_itself() {
    use std::collections::HashMap;
    // A triangle: its corners, and where on its texture each is.
    type Triangle = [(Vec3, [f32; 2]); 3];
    // How many triangles lie over an earlier one exactly, facing the same way, with the
    // texture laid on differently.
    fn clashes(triangles: impl Iterator<Item = Triangle>) -> usize {
        let mut seen: HashMap<[[u32; 3]; 3], [[u32; 2]; 3]> = HashMap::new();
        let mut clashes = 0;
        for triangle in triangles {
            let points = triangle.map(|(point, _)| point.to_array().map(f32::to_bits));
            let first = (0..3).min_by_key(|&k| points[k]).unwrap();
            let turned = [0, 1, 2].map(|k| (first + k) % 3);
            let looks = turned.map(|k| triangle[k].1.map(f32::to_bits));
            if let Some(earlier) = seen.insert(turned.map(|k| points[k]), looks) {
                clashes += (earlier != looks) as usize;
            }
        }
        clashes
    }
    // A face of the file as the importer draws its front: corners reversed, because it
    // flips Z, in metres, texture positions kept on the tile, and a fan from the first.
    fn front(model: &pod::Model, face: &pod::Face) -> Vec<Triangle> {
        let corners: Vec<(Vec3, [f32; 2])> = face
            .corners
            .iter()
            .rev()
            .map(|corner| {
                let [x, up, z] = model.vertices[corner.vertex];
                let uv = corner.uv.map(|along| {
                    if along.is_finite() {
                        along.clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                });
                (Vec3::new(x, up, -z) * 0.3048, uv)
            })
            .collect();
        (1..corners.len().saturating_sub(1))
            .map(|second| [corners[0], corners[second], corners[second + 1]])
            .collect()
    }

    for file in pods("tracks") {
        let archive = pod::PodArchive::parse(std::fs::read(&file).unwrap()).unwrap();
        let original = pod::Track::from_archive(&archive).unwrap();
        let track = load_pod(&file, &BaseGame::none()).unwrap();
        for model in &track.scenery.models {
            let Some(written) = original.models.get(&model.name.to_ascii_uppercase()) else {
                continue;
            };
            let in_the_file = clashes(written.faces.iter().flat_map(|face| front(written, face)));
            let drawn = clashes(model.indices.chunks(3).map(|triangle| {
                [0, 1, 2].map(|k| {
                    let index = triangle[k] as usize;
                    (Vec3::from(model.positions[index]), model.uvs[index])
                })
            }));
            assert!(
                drawn <= in_the_file,
                "{}: {} lays {drawn} triangles over others, differently textured; its file {in_the_file}",
                track.name,
                model.name
            );
        }
    }
}

/// Baja Beach's checkpoints have no model: each is an invisible box, `2,147,52.27` ft and
/// so on, and its `width` is how wide the gate is. Given a fixed width of one cell either
/// side instead, 8 of its 18 gates stood beside the route its computer trucks follow, and
/// the race put back every truck that kept to the route, from the second checkpoint on.
#[test]
fn a_checkpoint_with_no_model_is_as_wide_as_its_box() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/BAJBEACH_MTM2_HD.POD");
    if !path.exists() {
        return;
    }
    let track = load_pod(&path, &BaseGame::none()).unwrap();
    let feet = 0.3048;
    assert_eq!(track.gates.len(), 18);
    // The first in the file, which is gate 1 here: the finish line comes first.
    assert!((track.gates[1].half_width - 147.0 / 2.0 * feet).abs() < 0.01);

    let course = track.course.as_ref().unwrap();
    let route_misses: Vec<usize> = (0..track.gates.len())
        .filter(|&index| {
            let gate = &track.gates[index];
            let (direction, across) = (gate.direction(), gate.across());
            !course.segments().any(|(start, end)| {
                let (start, end) = (start - gate.center, end - gate.center);
                let (before, after) = (start.dot(direction), end.dot(direction));
                before < 0.0
                    && after >= 0.0
                    && start.lerp(end, before / (before - after)).dot(across).abs()
                        <= gate.half_width
            })
        })
        .collect();
    // These two stand 34 to 37 m to one side of a long straight piece of the route.
    assert_eq!(route_misses, [10, 14]);
}

/// Alpine's ice is where its ice textures are: 6 cells of 11EI7, 587 of 11EI4, 53 of 11EI5
/// and 762 of FDWATA1, read off its texture map. Its snow is firm.
#[test]
fn alpine_ice_is_where_its_ice_textures_are() {
    let Some(track) = alpine() else { return };
    assert_eq!(track.footing.len(), 256 * 256);
    let ice = track
        .footing
        .iter()
        .filter(|&&footing| footing == Footing::Ice);
    assert_eq!(ice.count(), 6 + 587 + 53 + 762);

    // The middle of each cell asks for that cell, whichever way the rows run.
    let cells = 256;
    let cell_size = track.heights.size() / cells as f32;
    let middle = |index: usize| {
        let (col, row) = (index % cells, index / cells);
        let at = |i: usize| (i as f32 + 0.5) * cell_size - track.heights.size() / 2.0;
        (at(col), at(row))
    };
    for (index, &footing) in track.footing.iter().enumerate() {
        let (x, z) = middle(index);
        assert_eq!(track.footing_at(x, z), footing, "cell {index}");
    }
    // Pole position is on firm ground.
    let start = track.start.position;
    assert_eq!(track.footing_at(start.x, start.y), Footing::Firm);
}

#[test]
fn my_track_has_no_ice() {
    let Some(track) = my_track() else { return };
    assert!(
        track
            .footing
            .iter()
            .all(|&footing| footing == Footing::Firm)
    );
}

/// Lands Between's ground textures each have a palette of their own. Through the track's,
/// they are coloured noise: 185 on average by this measure, against 18.
#[test]
fn lands_between_ground_takes_its_own_palettes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/landsbetween.pod");
    if !path.exists() {
        return;
    }
    let track = load_pod(&path, &BaseGame::none()).unwrap();
    let ground = track
        .ground
        .expect("Lands Between carries its ground textures");
    let size = ground.tile_size;
    let roughness = |rgba: &[u8]| {
        let pixels = rgba.as_chunks::<4>().0;
        let mut sum = 0.0;
        for row in 0..size {
            for col in 1..size {
                let (a, b) = (pixels[row * size + col - 1], pixels[row * size + col]);
                sum += (0..3)
                    .map(|i| (a[i] as f32 - b[i] as f32).abs())
                    .sum::<f32>();
            }
        }
        sum / (size * (size - 1)) as f32
    };
    let mean =
        ground.tiles.iter().map(|tile| roughness(tile)).sum::<f32>() / ground.tiles.len() as f32;
    assert!(mean < 50.0, "{mean}");
}

/// Solid boxes with a mass are loose, in kilograms. Alpine has 91, from a 9 kg sign to a
/// 136 t lorry.
#[test]
fn boxes_with_a_mass_are_loose() {
    use monster_truck_rural_ruckus::track::SceneryMotion;
    let Some(track) = alpine() else { return };
    let masses: Vec<f32> = track
        .scenery
        .objects
        .iter()
        .filter_map(|object| match object.motion {
            SceneryMotion::Loose { mass } => Some(mass),
            _ => None,
        })
        .collect();
    assert_eq!(masses.len(), 91);
    let (least, most) = masses.iter().fold((f32::MAX, f32::MIN), |(a, b), &mass| {
        (a.min(mass), b.max(mass))
    });
    assert!((least - 20.0 * 0.453_592).abs() < 0.1, "{least} kg");
    assert!((most - 300_000.0 * 0.453_592).abs() < 10.0, "{most} kg");
    assert!(
        track
            .scenery
            .objects
            .iter()
            .all(|object| object.solid || object.motion == SceneryMotion::Fixed)
    );
}

/// A moving object keeps its height, and does not follow the ground: every one found goes
/// along a level line that runs right across the map, where each dip in the ground is
/// bridged by a ground box level with it. Followed, the ground would take a train 20 m
/// down under its own bridge.
#[test]
fn moving_objects_go_along_a_level_line_across_the_map() {
    use monster_truck_rural_ruckus::track::SceneryMotion;
    for name in ["MonteCarlo_PZ.pod", "rute756jam.pod"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tracks")
            .join(name);
        if !path.exists() {
            continue;
        }
        let track = load_pod(&path, &BaseGame::none()).unwrap();
        let size = track.heights.size();
        let mut moving = 0;
        for object in &track.scenery.objects {
            let SceneryMotion::Moving { velocity } = object.motion else {
                continue;
            };
            moving += 1;
            assert_eq!(velocity.y, 0.0);
            let level = track
                .heights
                .height_at(object.position.x, object.position.y);
            // Once across the map, a point every metre of the way.
            let steps = (size / velocity.xz().length() * 60.0) as usize;
            for step in 0..steps {
                let at = object.position + velocity.xz() * step as f32 / 60.0;
                let at = (at + size / 2.0).rem_euclid(Vec2::splat(size)) - size / 2.0;
                let bridge = track
                    .ground_boxes
                    .iter()
                    .filter(|ground_box| {
                        let (low, high) = (
                            ground_box.min.min(ground_box.max),
                            ground_box.min.max(ground_box.max),
                        );
                        at.cmpge(low).all() && at.cmple(high).all()
                    })
                    .map(|ground_box| ground_box.top)
                    // Not a bridge overhead: Monte Carlo's lower train goes under its upper
                    // one's.
                    .filter(|&top| top < level + 1.0)
                    .fold(f32::MIN, f32::max);
                let under = track.heights.height_at(at.x, at.y).max(bridge);
                assert!(
                    (under - level).abs() < 0.5,
                    "{name}: {under} m under a moving object at {at}, which started at {level} m"
                );
            }
        }
        assert!(moving > 10, "{name}: {moving}");
    }
}
