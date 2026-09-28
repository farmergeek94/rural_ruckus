//! Checks the parsers against real archives, which can't be committed: they are read
//! from `tracks/` at the root of the repository, and the tests pass trivially when
//! there are none.

use std::path::Path;

use monster_truck_rural_ruckus::pod::{self, BoxShape, Model, PodArchive, Track};

fn real_archives() -> Vec<(String, PodArchive)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks");
    let Ok(files) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    files
        .flatten()
        .map(|file| file.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pod"))
        })
        .map(|path| {
            let archive = PodArchive::parse(std::fs::read(&path).unwrap());
            (path.display().to_string(), archive.unwrap())
        })
        .collect()
}

/// A list of tracks reads the name alone, and must call a track what loading it does.
#[test]
fn every_real_track_gives_its_name_without_being_loaded() {
    for (path, archive) in real_archives() {
        let name = Track::name_in(&archive).unwrap_or_else(|| panic!("{path}: no track"));
        assert_eq!(name, Track::from_archive(&archive).unwrap().situation.name);
        assert!(pod::Truck::name_in(&archive).is_none(), "{path}");
    }
}

#[test]
fn every_real_track_loads() {
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap_or_else(|error| panic!("{path}: {error}"));
        println!(
            "{path}: \"{}\", {} vehicles, {} boxes, {} checkpoints, {} course segments",
            track.situation.name,
            track.situation.vehicles.len(),
            track.situation.boxes.len(),
            track.situation.checkpoints().count(),
            track.situation.course.len(),
        );
        assert!(
            !track.situation.vehicles.is_empty(),
            "{path}: no starting grid"
        );
        // Every MTM2 terrain seen is 256 x 256. A smaller one means the heightmap's name
        // was matched to a ground texture of the same name in another folder.
        assert_eq!(track.heightmap.size(), 256, "{path}");
        assert!(
            track.texture_map.is_some(),
            "{path}: the texture map did not parse"
        );
        // Every archive seen has ground box files, though some have no boxes in them.
        let boxes = track.ground_boxes.as_ref();
        assert!(boxes.is_some(), "{path}: the ground boxes did not parse");
        assert!(
            boxes
                .unwrap()
                .iter()
                .all(|(.., found)| found.faces.is_some()),
            "{path}"
        );
        // Track editors set the grid down on the ground, a few feet up at most. Unless the
        // heightmap was changed after the track was laid out: `tground.pod` puts its grid
        // at 96 ft and its course at 68 to 96, under the lowest ground anywhere on its map
        // (162 ft). The game places trucks by X and Z alone, so such a track still races.
        let [x, y, z] = track.situation.vehicles[0].position;
        let size = track.heightmap.size();
        let lowest_ground = (0..size * size)
            .map(|index| track.heightmap.feet(index % size, index / size))
            .fold(f32::INFINITY, f32::min);
        if y < lowest_ground {
            println!("{path}: laid out on other ground, the grid is under all of it");
            continue;
        }
        let above = y - track.heightmap.ground_feet(x, z);
        assert!(
            (0.0..20.0).contains(&above),
            "{path}: pole position is {above} ft above the ground"
        );
    }
}

/// Holds both `ART\MONTECAR.RAW`, a ground texture, and `DATA\MONTECAR.RAW`, the
/// heightmap. The level names `montecar.raw`, and must get the heightmap.
#[test]
fn monte_carlo_reads_its_heightmap_and_not_the_texture_of_the_same_name() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("MonteCarlo_PZ.pod"))
    else {
        return;
    };
    assert_eq!(archive.find_file("montecar.raw").unwrap().size, 4096);
    assert_eq!(
        archive.find_file_in("DATA", "montecar.raw").unwrap().name,
        "DATA\\MONTECAR.RAW"
    );

    let track = Track::from_archive(&archive).unwrap();
    assert_eq!(track.situation.name, "Monte Carlo");
    assert_eq!(track.heightmap.size(), 256);
    assert!(track.texture_map.is_some());
    // The grid is on a hilltop 460 ft up, 8 ft above the ground it was placed on.
    let [x, y, z] = track.situation.vehicles[0].position;
    assert_eq!(y, 460.0);
    assert_eq!(track.heightmap.ground_feet(x, z), 452.0);
}

/// Converted from 4x4 Evolution 2 (its archive says so) for MTM2 Community Patch 3. Its
/// track file is `WORLD\BAJBEACH.SI2`, with the same labels as a .SIT, and 4096 boxes
/// where MTM2's own tracks have a few hundred. Its textures are PNGs only, and its models
/// use material records.
#[test]
fn baja_beach_reads_its_si2_track_file() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("BAJBEACH_MTM2_HD.POD"))
    else {
        return;
    };
    let track = Track::from_archive(&archive).unwrap();
    let situation = &track.situation;
    assert_eq!(situation.name, "Baja Beach");
    assert_eq!(situation.level_file, "BAJBEACH.LVL");
    assert_eq!(situation.vehicles.len(), 8);
    assert_eq!(situation.boxes.len(), 4096);
    assert_eq!(situation.checkpoints().count(), 18);
    assert_eq!(situation.course.len(), 26);
    // The grid stands on the ground, as in MTM2's own tracks: the heights agree.
    let [x, y, z] = situation.vehicles[0].position;
    assert_eq!([x, y, z], [6561.95, 135.63, 4497.98]);
    assert!((0.0..20.0).contains(&(y - track.heightmap.ground_feet(x, z))));

    // Every texture is a PNG, square, and found by its .RAW name.
    assert!(track.ground_textures.iter().all(Option::is_none));
    assert_eq!(track.ground_hd_textures.len(), 191);
    assert!(
        track
            .ground_hd_textures
            .iter()
            .all(|texture| texture.as_ref().is_some_and(|texture| texture.size() == 64))
    );
    assert!(track.model_textures.is_empty());
    assert_eq!(track.model_hd_textures.len(), 23);
    assert_eq!(track.model_hd_textures["PALMFAN.RAW"].size(), 256);

    // Each model reads to its end: a material record (63), then faces of type 64 that
    // take it. The palm's leaves are cut out of their texture and seen from both sides.
    assert_eq!(track.models.len(), 23);
    for (name, model) in &track.models {
        assert_eq!(model.incomplete, None, "{name}");
        assert!(!model.faces.is_empty(), "{name}");
    }
    let palm = &track.models["V_PALMFAN.BIN"];
    assert!(palm.faces.iter().all(|face| face.kind == 64));
    assert!(
        palm.faces
            .iter()
            .any(|face| face.is_cutout() && face.is_two_sided())
    );
}

/// Facts read out of this particular archive by hand, with a hex dump and a text editor.
#[test]
#[allow(
    clippy::excessive_precision,
    reason = "the numbers are as the file writes them"
)]
fn my_track_matches_what_is_in_the_file() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("MyTrack.pod"))
    else {
        return;
    };
    assert_eq!(archive.comment(), "Tracked2 School by Phineus");
    assert_eq!(archive.entries().len(), 20);
    assert_eq!(archive.find_file("nutrack.act").unwrap().size, 768);

    let track = Track::from_archive(&archive).unwrap();
    assert_eq!(track.situation.name, "My First Track");
    assert_eq!(track.level.heightmap_file, "nutrack.raw");

    assert_eq!(track.situation.vehicles.len(), 8);
    assert_eq!(
        track.situation.vehicles[0].position,
        [4338.277344, 126.0, 3887.125]
    );
    assert_eq!(track.situation.vehicles[7].truck_file, "HITMAN.TRK");

    assert_eq!(track.situation.boxes.len(), 9);
    let checkpoints: Vec<_> = track.situation.checkpoints().collect();
    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0].shape, BoxShape::Model("CKBOXN.BIN".into()));
    assert_eq!(
        checkpoints[1].position,
        [4353.0625, 135.998047, 3964.242188]
    );
    assert_eq!(track.situation.course.len(), 4);

    // The ground under the start line is 60 steps, 120 ft, matching the course's height.
    assert_eq!(track.heightmap.size(), 256);
    assert_eq!(track.heightmap.feet(4348 / 32, 3940 / 32), 120.0);
    assert_eq!(track.situation.course[0].start[1], 120.0);

    // The road along x = 3840 ft is two cells wide, centred on the line between
    // columns 119 and 120. This is what fixes rows to Z and columns to X.
    let map = track.texture_map.unwrap();
    let grass = map.most_common_texture();
    for row in 125..130 {
        assert_eq!(map.cell(118, row).texture, grass);
        assert_ne!(map.cell(119, row).texture, grass);
        assert_ne!(map.cell(120, row).texture, grass);
        assert_eq!(map.cell(121, row).texture, grass);
    }
}

/// The strongest evidence for how terrain works. Track editors set objects down on the
/// ground, so each model sits at its own fixed height above it, however steep the
/// mountain. That only comes out if the height scale, the row and column order and the
/// way cells are split into triangles are all right: with the wrong diagonal in a
/// cell the error is feet, not thousandths of a foot.
#[test]
fn alpine_scenery_sits_on_the_ground() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("AlpineMtns.pod"))
    else {
        return;
    };
    let track = Track::from_archive(&archive).unwrap();

    for (model, expected_height, count) in [
        ("GRDRAIL2.BIN", 3.0, 33),
        ("TIRRV21.BIN", 2.5, 4),
        ("CASSIGA.BIN", 10.0, 20),
    ] {
        let heights: Vec<f32> = track
            .situation
            .boxes
            .iter()
            .filter(|scenery| scenery.shape == BoxShape::Model(model.into()))
            .map(|scenery| {
                let [x, y, z] = scenery.position;
                y - track.heightmap.ground_feet(x, z)
            })
            .collect();
        assert_eq!(heights.len(), count, "{model}");

        let mut sorted = heights.clone();
        sorted.sort_by(f32::total_cmp);
        let median = sorted[sorted.len() / 2];
        assert!(
            (median - expected_height).abs() < 0.01,
            "{model} sits {median} ft up"
        );

        // A few were nudged by hand, so ask for most rather than all.
        let exact = heights
            .iter()
            .filter(|height| (*height - median).abs() < 0.02)
            .count();
        assert!(
            exact * 10 >= count * 8,
            "{model}: only {exact} of {count} sit at {median} ft"
        );
    }
}

#[test]
fn alpine_checkpoints_and_grid() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("AlpineMtns.pod"))
    else {
        return;
    };
    let track = Track::from_archive(&archive).unwrap();
    assert_eq!(track.situation.name, "Alpine Mountains");

    // Ten checkpoints. The last in the file is the finish line, right by the grid; the
    // others are all at least a quarter of a mile away.
    let pole = track.situation.vehicles[0].position;
    let distances: Vec<f32> = track
        .situation
        .checkpoints()
        .map(|checkpoint| {
            (checkpoint.position[0] - pole[0]).hypot(checkpoint.position[2] - pole[2])
        })
        .collect();
    assert_eq!(distances.len(), 10);
    assert!(
        distances[9] < 300.0,
        "finish is {} ft from pole",
        distances[9]
    );
    assert!(distances[..9].iter().all(|&distance| distance > 1400.0));
}

/// How textures are laid on cells was worked out by trying every combination of which
/// way up they start, which way they turn, what the mirror bits flip and in what order,
/// and keeping the one where neighbouring tiles meet most smoothly. This holds the
/// winner against its nearest rivals: get any one of them wrong and roads stop lining up,
/// which shows as a much bigger jump in colour across the edges between cells.
#[test]
fn alpine_ground_tiles_line_up_with_their_neighbours() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("AlpineMtns.pod"))
    else {
        return;
    };
    let track = Track::from_archive(&archive).unwrap();
    let map = track.texture_map.as_ref().unwrap();
    let palette = track.palette.as_ref().unwrap();
    assert_eq!(track.ground_textures.len(), 100);
    assert!(
        track
            .ground_textures
            .iter()
            .all(|texture| { texture.as_ref().is_some_and(|texture| texture.size() == 64) })
    );

    type Placement = fn(&pod::TextureCell, f32, f32) -> (f32, f32);
    let measured: Placement = |cell, u, v| cell.texture_coords(u, v);
    let turned_the_other_way: Placement = |cell, u, v| {
        let reversed = pod::TextureCell {
            rotation: (4 - cell.rotation) % 4,
            ..*cell
        };
        reversed.texture_coords(u, v)
    };
    let mirror_bits_swapped: Placement = |cell, u, v| {
        let swapped = pod::TextureCell {
            mirror: (cell.mirror >> 1) | (cell.mirror & 1) << 1,
            ..*cell
        };
        swapped.texture_coords(u, v)
    };
    let upside_down: Placement = |cell, u, v| {
        let (across, down) = cell.texture_coords(u, v);
        (across, 1.0 - down)
    };

    // Mean difference in colour, per channel, across every edge between two cells of
    // which at least one is mirrored, or (unless `mirrored_only`) turned. Few cells are
    // mirrored, so the mirror bits only show when the rest are left out.
    let jump = |place: Placement, mirrored_only: bool| {
        let colour = |column: usize, row: usize, u: f32, v: f32| {
            let cell = map.cell(column, row);
            let texture = track.ground_textures[cell.texture as usize]
                .as_ref()
                .unwrap();
            let (across, down) = place(&cell, u, v);
            let pixel = |along: f32| ((along * 64.0) as usize).min(63);
            palette.color(texture.index(pixel(across), pixel(down)))
        };
        let (mut total, mut count) = (0.0, 0.0);
        for row in 0..map.size() {
            for column in 0..map.size() {
                for (next_column, next_row) in [(column + 1, row), (column, row + 1)] {
                    let (here, there) = (map.cell(column, row), map.cell(next_column, next_row));
                    let mirrored = here.mirror + there.mirror > 0;
                    let turned = here.rotation + there.rotation > 0;
                    if !mirrored && (mirrored_only || !turned) {
                        continue;
                    }
                    for step in 0..64 {
                        let along = (step as f32 + 0.5) / 64.0;
                        let (a, b) = if next_row == row {
                            (
                                colour(column, row, 0.999, along),
                                colour(next_column, row, 0.001, along),
                            )
                        } else {
                            (
                                colour(column, row, along, 0.999),
                                colour(column, next_row, along, 0.001),
                            )
                        };
                        for channel in 0..3 {
                            total += (a[channel] as f32 - b[channel] as f32).abs();
                            count += 1.0;
                        }
                    }
                }
            }
        }
        total / count
    };

    for (name, rival, mirrored_only) in [
        ("turned the other way", turned_the_other_way, false),
        ("upside down", upside_down, false),
        ("mirror bits swapped", mirror_bits_swapped, true),
    ] {
        let (best, worse) = (jump(measured, mirrored_only), jump(rival, mirrored_only));
        println!("measured {best:.1}, {name} {worse:.1}");
        assert!(worse > best * 1.3, "{name} scores {worse}, measured {best}");
    }
}

fn alpine() -> Option<(PodArchive, Track)> {
    let (_, archive) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("AlpineMtns.pod"))?;
    let track = Track::from_archive(&archive).unwrap();
    Some((archive, track))
}

#[test]
fn every_alpine_model_reads_to_its_end() {
    let Some((archive, track)) = alpine() else {
        return;
    };
    let mut faces = 0;
    for entry in archive.entries() {
        if !entry.extension().eq_ignore_ascii_case("bin") {
            continue;
        }
        let model = Model::parse(archive.data(entry))
            .unwrap_or_else(|error| panic!("{}: {error}", entry.name));
        assert_eq!(model.incomplete, None, "{}", entry.name);
        assert!(
            !model.vertices.is_empty() && !model.faces.is_empty(),
            "{}",
            entry.name
        );
        faces += model.faces.len();
    }
    assert_eq!(faces, 3464);

    // Every model a box names is in the archive, and every texture those use.
    // All but the backdrop, which the track names elsewhere.
    assert_eq!(track.models.len(), 31);
    assert!(!track.models.contains_key("11EDROP.BIN"));
    for model in track.models.values() {
        for texture in model.textures() {
            let texture = &track.model_textures[&texture.to_ascii_uppercase()];
            assert_eq!(texture.size(), 64);
        }
    }
}

/// Ties the models to the terrain. Each model was measured, from the heightmap and the
/// track file alone, to sit a fixed height above the ground (see
/// `alpine_scenery_sits_on_the_ground`). That height turns out to be exactly how far the
/// model reaches below its own origin: the editor stands models on their lowest point.
/// It only comes out if vertices are 256 to the foot and the second number is "up".
#[test]
fn alpine_models_reach_down_to_the_ground_they_stand_on() {
    let Some((_, track)) = alpine() else { return };
    for (name, measured_height) in [
        ("GRDRAIL2.BIN", 3.0),
        ("13ETREE1.BIN", 11.0),
        ("13ETREE2.BIN", 6.0),
        ("CASSIGA.BIN", 10.0),
        ("DOZER2.BIN", 8.0),
        ("TIRRV21.BIN", 2.5),
        ("18ESNCAT.BIN", 6.0),
    ] {
        let (low, _) = track.models[name].bounds();
        assert!(
            (-low[1] - measured_height).abs() < 0.02,
            "{name} reaches {} ft down",
            -low[1]
        );
    }

    // The finish line's trigger box is 192 ft wide, 32 deep and 32 high.
    let (low, high) = track.models["CKBOX.BIN"].bounds();
    assert_eq!((low, high), ([-96.0, -16.0, -16.0], [96.0, 16.0, 16.0]));
}

/// Which way a model turns with its heading. Guard rails are long along their X axis and
/// are laid in lines, so under the right rule a rail points at its neighbour; and a
/// checkpoint banner, also long in X, lies across the course.
#[test]
fn alpine_models_turn_with_their_heading() {
    let Some((_, track)) = alpine() else { return };
    let x_axis = |psi: f32| [psi.cos(), -psi.sin()];
    let dot = |a: [f32; 2], b: [f32; 2]| a[0] * b[0] + a[1] * b[1];
    let named = |prefix: &'static str| {
        track.situation.boxes.iter().filter(move |scenery| {
            matches!(&scenery.shape, BoxShape::Model(name) if name.starts_with(prefix))
        })
    };

    let rails: Vec<_> = named("GRDRAIL2").collect();
    let (mut with_neighbour, mut aligned) = (0, 0);
    for rail in &rails {
        let to_nearest = rails
            .iter()
            .filter(|other| other.position != rail.position)
            .map(|other| {
                [
                    other.position[0] - rail.position[0],
                    other.position[2] - rail.position[2],
                ]
            })
            .min_by(|a, b| dot(*a, *a).total_cmp(&dot(*b, *b)))
            .unwrap();
        let distance = dot(to_nearest, to_nearest).sqrt();
        if distance < 60.0 {
            with_neighbour += 1;
            if dot(x_axis(rail.angles[2]), to_nearest).abs() / distance > 0.95 {
                aligned += 1;
            }
        }
    }
    assert!(
        with_neighbour >= 30 && aligned * 10 >= with_neighbour * 8,
        "{aligned} of {with_neighbour}"
    );

    for banner in named("PEPBAN") {
        let [x, _, z] = banner.position;
        let along_course = track
            .situation
            .course
            .iter()
            .map(|segment| {
                let (a, b) = (segment.start, segment.end);
                let along = [b[0] - a[0], b[2] - a[2]];
                let length = dot(along, along).sqrt();
                let t = (dot([x - a[0], z - a[2]], along) / (length * length)).clamp(0.0, 1.0);
                let gap = [x - a[0] - along[0] * t, z - a[2] - along[1] * t];
                (dot(gap, gap), [along[0] / length, along[1] / length])
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap()
            .1;
        assert!(
            dot(x_axis(banner.angles[2]), along_course).abs() < 0.3,
            "banner at ({x}, {z})"
        );
    }
}

/// Past checkpoint 6, Alpine's course crosses a ravine 120 ft deep. The ground boxes are a
/// deck over it, level with the ground where it meets it at both ends. This is also what
/// says where a box stands: on the texture map's cell of the same number, from sample
/// (column, row) to (column + 1, row + 1). Half a cell back, the deck would stop short
/// over a drop of 50 ft.
#[test]
fn alpine_has_a_bridge_over_the_ravine() {
    let Some((_, track)) = alpine() else { return };
    let boxes = track.ground_boxes.as_ref().unwrap();
    let heightmap = &track.heightmap;
    assert_eq!(boxes.iter().count(), 502);

    for column in 39..=41 {
        for row in 185..=192 {
            let deck = boxes.get(column, row).unwrap();
            assert_eq!((deck.lower, deck.upper), (119, 120), "cell {column}, {row}");
        }
        // Nothing either side of it along the course.
        assert_eq!(boxes.get(column, 184), None);
        assert_eq!(boxes.get(column, 193), None);
    }
    // Where the deck ends, the ground is as high as its top, all the way across; under
    // its middle it is 120 ft further down.
    for column in 39..=42 {
        for row in [185, 193] {
            assert_eq!(heightmap.steps(column, row), 120, "sample {column}, {row}");
        }
        assert_eq!(heightmap.steps(column, 190), 58);
    }
}

/// Where a box stands, scored over every real track: of the edges where a box meets the
/// ground at nearly its own height, how many meet it exactly. Decks and ramps are built to
/// be driven on and off, so under the right rule most of them do.
#[test]
fn ground_boxes_meet_the_ground_on_the_texture_maps_cells() {
    let flush_edges = |offset: f32| {
        let (mut flush, mut edges) = (0, 0);
        for (_, archive) in real_archives() {
            let track = Track::from_archive(&archive).unwrap();
            let (heightmap, boxes) = (&track.heightmap, track.ground_boxes.as_ref().unwrap());
            let size = boxes.size();
            for (column, row, found) in boxes.iter() {
                let top = found.upper as f32 * pod::FEET_PER_HEIGHT_STEP;
                let (x, z) = (
                    (column as f32 + offset) * pod::FEET_PER_CELL,
                    (row as f32 + offset) * pod::FEET_PER_CELL,
                );
                let half = pod::FEET_PER_CELL / 2.0;
                let full = pod::FEET_PER_CELL;
                // The middle of each side, and the cell beyond it.
                for (dx, dz, (mx, mz)) in [
                    (1, 0, (x + full, z + half)),
                    (size - 1, 0, (x, z + half)),
                    (0, 1, (x + half, z + full)),
                    (0, size - 1, (x + half, z)),
                ] {
                    if boxes.get(column + dx, row + dz).is_some() {
                        continue;
                    }
                    let gap = (heightmap.ground_feet(mx, mz) - top).abs();
                    if gap <= 10.0 {
                        edges += 1;
                        flush += usize::from(gap <= 1.0);
                    }
                }
            }
        }
        flush as f32 / edges.max(1) as f32
    };
    let measured = flush_edges(0.0);
    println!(
        "flush: {:.2} on the cell, {:.2} half a cell back, {:.2} a cell back",
        measured,
        flush_edges(-0.5),
        flush_edges(-1.0)
    );
    for rival in [-0.5, -1.0] {
        assert!(measured >= flush_edges(rival), "offset {rival} fits better");
    }
}

/// The water height's scale (`docs/formats/level.md`). Read as it is written, or halved,
/// it drowns starting grids; divided by more than four, Baja's corals stand in the air.
/// As half feet, every grid is dry, every coral is under water, and a raft floats on it.
#[test]
fn real_water_is_below_the_grid_and_over_the_coral() {
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap();
        let Some(water) = track.level.water_feet().filter(|&feet| feet > 0.0) else {
            continue;
        };
        let ground = |position: [f32; 3]| track.heightmap.ground_feet(position[0], position[2]);
        for vehicle in &track.situation.vehicles {
            assert!(
                ground(vehicle.position) > water,
                "{path}: a place on the grid is under {water} ft of water"
            );
        }
        for situation_box in &track.situation.boxes {
            let BoxShape::Model(model) = &situation_box.shape else {
                continue;
            };
            let model = model.to_ascii_uppercase();
            let [_, y, _] = situation_box.position;
            if model.contains("CORAL") {
                assert!(
                    y < water,
                    "{path}: {model} at {y} ft, over {water} ft of water"
                );
            }
            if model.contains("RAFT") {
                assert!(
                    (0.0..10.0).contains(&(y - water)),
                    "{path}: {model} at {y} ft does not float on water at {water} ft"
                );
            }
        }
    }
}

/// How rough a picture is: the mean difference between neighbouring pixels across it,
/// leaving out black, which a cutout face makes a hole. Through the wrong palette a
/// picture turns to coloured noise, which is far rougher than any real one.
fn roughness(rgba: &[u8], size: usize) -> f32 {
    let pixels: Vec<&[u8; 4]> = rgba.as_chunks::<4>().0.iter().collect();
    let (mut sum, mut count) = (0.0, 0);
    for row in 0..size {
        for col in 1..size {
            let (a, b) = (pixels[row * size + col - 1], pixels[row * size + col]);
            if a[..3] == [0, 0, 0] || b[..3] == [0, 0, 0] {
                continue;
            }
            sum += (0..3)
                .map(|i| (a[i] as f32 - b[i] as f32).abs())
                .sum::<f32>();
            count += 1;
        }
    }
    sum / count.max(1) as f32
}

/// A texture with a palette of the same name beside it is coloured by that palette.
/// Measured: through the track's palette, every backdrop texture is noise, and so is the
/// ground of Lands Between, Route 77, Rute 756 jam, tground and The Tight Corners, all of
/// whose ground textures have palettes of their own. Their scenery is rougher too. A few
/// own palettes (Monte Carlo's ground) are the track's, and make no difference.
#[test]
fn textures_take_their_own_palette() {
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap();
        let Some(track_palette) = &track.palette else {
            continue;
        };
        let backdrop: Vec<String> = track
            .backdrop_models
            .values()
            .flat_map(Model::textures)
            .map(str::to_ascii_uppercase)
            .collect();
        let ground_texture = |name: &str| {
            let index = track
                .ground_texture_names
                .iter()
                .position(|ground| ground.eq_ignore_ascii_case(name))?;
            track.ground_textures.get(index)?.as_ref()
        };
        let (mut own_sum, mut theirs_sum) = (0.0, 0.0);
        for (name, own) in &track.own_palettes {
            let Some(texture) = track
                .model_textures
                .get(name)
                .or_else(|| ground_texture(name))
            else {
                continue;
            };
            let size = texture.size();
            let [own, theirs] =
                [own, track_palette].map(|palette| roughness(&texture.to_rgba(palette), size));
            if backdrop.contains(name) {
                assert!(own * 2.0 < theirs, "{path}: {name}: {own} against {theirs}");
            }
            own_sum += own;
            theirs_sum += theirs;
        }
        println!("{path}: {own_sum:.0} through their own palettes, {theirs_sum:.0} the track's");
        assert!(
            own_sum <= theirs_sum,
            "{path}: {own_sum} against {theirs_sum}"
        );
    }
}

/// Every backdrop face seen is a cutout: the black of a backdrop's texture is the sky.
#[test]
fn backdrops_are_cutouts_round_the_origin() {
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap();
        for (name, model) in &track.backdrop_models {
            assert!(
                model.faces.iter().all(|face| face.is_cutout()),
                "{path}: {name}"
            );
            // A ring round its origin, in feet, reaching both above and below it.
            let up = model.vertices.iter().map(|vertex| vertex[1]);
            let (low, high) = up.fold((f32::MAX, f32::MIN), |(l, h), y| (l.min(y), h.max(y)));
            assert!(low < 0.0 && high > 0.0, "{path}: {name}: {low} to {high}");
            let out = model
                .vertices
                .iter()
                .map(|[x, _, z]| x.hypot(*z))
                .fold(0.0, f32::max);
            assert!(
                (150.0..250.0).contains(&out),
                "{path}: {name}: {out} ft out"
            );
        }
    }
}

/// The level file doesn't name the texture types: they are found by the heightmap's name.
#[test]
fn every_real_track_with_texture_types_reads_them() {
    for (path, archive) in real_archives() {
        let has_file = archive
            .entries()
            .iter()
            .any(|entry| entry.extension().eq_ignore_ascii_case("tty"));
        let track = Track::from_archive(&archive).unwrap();
        assert_eq!(track.texture_types.is_some(), has_file, "{path}");
        if let Some(types) = &track.texture_types {
            println!("{path}: {} texture types", types.entries().len());
        }
    }
}

/// Which of two types a texture listed twice takes is not known, so it must not matter.
#[test]
fn no_ground_is_a_texture_listed_with_two_types() {
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap();
        let (Some(types), Some(map)) = (&track.texture_types, &track.texture_map) else {
            continue;
        };
        let two_types = |name: &str| {
            let mut classes = types
                .entries()
                .iter()
                .filter(|entry| entry.texture.eq_ignore_ascii_case(name))
                .map(|entry| entry.class);
            let first = classes.next();
            classes.any(|class| Some(class) != first)
        };
        for row in 0..map.size() {
            for col in 0..map.size() {
                let texture = map.cell(col, row).texture as usize;
                let Some(name) = track.ground_texture_names.get(texture) else {
                    continue;
                };
                assert!(!two_types(name), "{path}: {name} at {col}, {row}");
            }
        }
    }
}

/// Types 800 to 999 are taken for ice. Every texture of those types in the archives seen
/// was looked at through its palette, and is ice: if another turns up, look at it.
#[test]
fn the_ice_types_are_the_ice_textures() {
    let ice = [
        "11EI4.RAW",
        "11EI5.RAW",
        "11EI7.RAW",
        "FDWATA1.RAW",
        "SNWICE11.RAW",
    ];
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap();
        let Some(types) = &track.texture_types else {
            continue;
        };
        for entry in types.entries() {
            if (800..=999).contains(&entry.class) {
                let name = entry.texture.to_ascii_uppercase();
                assert!(
                    ice.contains(&name.as_str()),
                    "{path}: {name} is {}",
                    entry.class
                );
            }
        }
    }
    if let Some((_, track)) = alpine() {
        let types = track.texture_types.unwrap();
        assert_eq!(types.class_of("11EI7.RAW"), Some(800));
        assert_eq!(types.class_of("11EI4.RAW"), Some(901));
        assert_eq!(types.class_of("FDWATA1.RAW"), Some(902));
        // Snow, the most of the ground.
        assert_eq!(types.class_of("11EC024.RAW"), Some(601));
    }
}

/// A box's mass is in slugs, pounds over g: every mass in Alpine is a round number of
/// pounds once multiplied by g, from a 20 lb sign to a 300 000 lb lorry.
#[test]
fn alpine_masses_are_round_pounds() {
    let Some((_, track)) = alpine() else { return };
    let masses: Vec<f32> = track
        .situation
        .boxes
        .iter()
        .map(|situation_box| situation_box.mass)
        .filter(|&mass| mass > 0.0)
        .collect();
    assert_eq!(masses.len(), 91);
    const G_IN_FEET: f32 = 32.174;
    for mass in masses {
        let pounds = mass * G_IN_FEET;
        assert!((pounds - pounds.round()).abs() < 0.1, "{mass} slugs");
        assert!((pounds.round() as u32).is_multiple_of(5), "{pounds} lb");
    }
}

/// `bvel` is along the world's axes, not the box's own. Monte Carlo has two trains, and
/// each lies in a line along its own velocity with its locomotive in front. Read in the
/// box's own axes, the train whose cars have a heading of 90 degrees would go sideways.
#[test]
fn monte_carlo_trains_run_along_their_line_locomotive_first() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tracks/MonteCarlo_PZ.pod");
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let track = Track::from_archive(&PodArchive::parse(bytes).unwrap()).unwrap();
    let moving: Vec<_> = track
        .situation
        .boxes
        .iter()
        .filter(|situation_box| situation_box.kind == pod::box_type::MOVING)
        .collect();
    assert_eq!(moving.len(), 11);
    let velocities: std::collections::BTreeSet<[i32; 3]> = moving
        .iter()
        .map(|situation_box| situation_box.velocity.map(|along| along as i32))
        .collect();
    assert_eq!(velocities.len(), 2, "two trains");
    for velocity in velocities {
        let train: Vec<_> = moving
            .iter()
            .filter(|situation_box| situation_box.velocity.map(|along| along as i32) == velocity)
            .collect();
        let way = velocity.map(|along| along as f32);
        let speed = (way[0] * way[0] + way[2] * way[2]).sqrt();
        let (ahead, across): (Vec<f32>, Vec<f32>) = train
            .iter()
            .map(|car| {
                let [x, _, z] = car.position;
                (
                    (x * way[0] + z * way[2]) / speed,
                    (x * way[2] - z * way[0]) / speed,
                )
            })
            .unzip();
        // In a line: within a couple of feet of each other across the way they go.
        let spread = |values: &[f32]| {
            values.iter().copied().fold(f32::MIN, f32::max)
                - values.iter().copied().fold(f32::MAX, f32::min)
        };
        assert!(spread(&across) < 2.0, "{velocity:?}: {across:?}");
        assert!(spread(&ahead) > 100.0, "{velocity:?}: {ahead:?}");
        // The locomotive is the one furthest along the way the train goes.
        let front = (0..train.len())
            .max_by(|&a, &b| ahead[a].total_cmp(&ahead[b]))
            .unwrap();
        assert_eq!(train[front].shape, BoxShape::Model("LOCOMTV.BIN".into()));
    }
}

/// Every record of a model read straight through, one after another, stepping over Jump
/// and Order records by their lengths (**reference**, JSTrackViewer), as (offset, type), up
/// to and with the first end record.
fn records_read_straight_through(bytes: &[u8]) -> Vec<(usize, i32)> {
    let int = |offset: usize| i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let mut records = Vec::new();
    let mut offset = 0;
    while offset + 4 <= bytes.len() {
        let kind = int(offset);
        records.push((offset, kind));
        let words = match kind {
            0 => break,
            2 | 3 => 3 + 3 * int(offset + 8) as usize,
            29 => 7 + 8 * int(offset + 8) as usize,
            24 | 17 | 41 | 51 | 64 => 6 + 3 * int(offset + 4) as usize,
            25 | 5 => 6 + int(offset + 4) as usize,
            12 => 7,
            13 => 6,
            10 | 18 | 20 => 2,
            other => panic!("record {other} at {offset}"),
        };
        offset += words * 4;
    }
    records
}

/// Alpine's helicopter is the one model with Order and Jump records. Stepped over, they
/// leave every face read, once, and the rotor's faces take its animated texture. Each
/// Jump's number, and the Order's last two, added to where the record starts, land on the
/// start of a record: they say where to go on, which the game here does not need.
#[test]
fn the_helicopter_reads_every_face_with_its_order_and_jumps_stepped_over() {
    let Some((archive, _)) = alpine() else {
        return;
    };
    let entry = archive.find_file_in("MODELS", "HELI.BIN").unwrap();
    let bytes = archive.data(entry);
    let model = Model::parse(bytes).unwrap();
    assert_eq!(model.incomplete, None);
    let records = records_read_straight_through(bytes);
    let face_kinds: Vec<i32> = records
        .iter()
        .map(|&(_, kind)| kind)
        .filter(|kind| ![0, 2, 3, 10, 12, 13, 18, 20, 29].contains(kind))
        .collect();
    assert_eq!(
        model.faces.iter().map(|face| face.kind).collect::<Vec<_>>(),
        face_kinds
    );

    let int = |offset: usize| i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let starts: Vec<usize> = records.iter().map(|&(offset, _)| offset).collect();
    let lands =
        |from: usize, by: i32| starts.contains(&from.checked_add_signed(by as isize).unwrap());
    let jumps: Vec<usize> = records.iter().filter(|r| r.1 == 18).map(|r| r.0).collect();
    let orders: Vec<usize> = records.iter().filter(|r| r.1 == 12).map(|r| r.0).collect();
    assert_eq!((jumps.len(), orders.len()), (3, 1));
    for jump in jumps {
        assert!(lands(jump, int(jump + 4)), "jump at {jump}");
    }
    for field in [20, 24] {
        assert!(
            lands(orders[0], int(orders[0] + field)),
            "order field {field}"
        );
    }

    assert_eq!(model.texture_cycles.len(), 1);
    let rotor = &model.texture_cycles[0];
    let names: Vec<String> = (1..=8).map(|frame| format!("rotor{frame}.raw")).collect();
    assert_eq!(rotor.frames, names);
    // Its rate, 1024, read as the game here reads it.
    assert_eq!(rotor.seconds_per_frame, 1.0 / 64.0);
    let turning = model
        .faces
        .iter()
        .filter(|face| face.texture_cycle.is_some());
    assert!(turning.count() > 0);
}

/// Every animated texture of every track has all its frames in the archive, and each is
/// shown for some time. No track has more than the scenery's material can show.
#[test]
fn every_animated_texture_has_its_frames() {
    for (path, archive) in real_archives() {
        let track = Track::from_archive(&archive).unwrap();
        let mut cycles = 0;
        for (name, model) in &track.models {
            for cycle in &model.texture_cycles {
                assert!(cycle.frames.len() >= 2, "{path} {name}");
                assert!(cycle.seconds_per_frame > 0.0, "{path} {name}");
                for frame in &cycle.frames {
                    let key = frame.to_ascii_uppercase();
                    assert!(
                        track.model_textures.contains_key(&key)
                            || track.model_hd_textures.contains_key(&key),
                        "{path} {name}: {frame}"
                    );
                }
                println!(
                    "{path} {name}: {:?} at {} s",
                    cycle.frames, cycle.seconds_per_frame
                );
                cycles += 1;
            }
        }
        assert!(cycles <= 3, "{path}: {cycles}");
    }
}

/// Critic's animated billboard: eight frames of 48 vertices, 1.25 s apart.
/// Every model of a box that always faces the camera (type 8) is a flat picture, upright
/// in the model's X and Y and drawn on both sides: half its faces are wound to be seen
/// from +Z and half from -Z. So it can be turned about Y alone, either side to the camera.
#[test]
fn what_faces_the_camera_is_a_flat_picture_drawn_on_both_sides() {
    for (path, archive) in real_archives() {
        let Ok(track) = Track::from_archive(&archive) else {
            continue;
        };
        for situation_box in &track.situation.boxes {
            let BoxShape::Model(name) = &situation_box.shape else {
                continue;
            };
            if situation_box.kind != 8 {
                continue;
            }
            let Some(model) = track.models.get(&name.to_ascii_uppercase()) else {
                continue;
            };
            let depth = model
                .vertices
                .iter()
                .map(|v| v[2].abs())
                .fold(0.0, f32::max);
            let height = model
                .vertices
                .iter()
                .map(|v| v[1].abs())
                .fold(0.0, f32::max);
            assert!(depth < 0.2 && height > 10.0, "{path} {name}");
            let (mut towards_plus_z, mut towards_minus_z) = (0, 0);
            for face in &model.faces {
                let corners: Vec<[f32; 3]> = face
                    .corners
                    .iter()
                    .map(|corner| model.vertices[corner.vertex])
                    .collect();
                // The Z of the face's normal by its winding (Newell's method).
                let mut z = 0.0;
                for (i, a) in corners.iter().enumerate() {
                    let b = corners[(i + 1) % corners.len()];
                    z += (a[0] - b[0]) * (a[1] + b[1]);
                }
                if z > 0.0 {
                    towards_plus_z += 1;
                } else {
                    towards_minus_z += 1;
                }
            }
            assert_eq!(towards_plus_z, towards_minus_z, "{path} {name}");
        }
    }
}

#[test]
fn critic_has_a_billboard_that_moves_by_keyframes() {
    let Some((_, archive)) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with("Critic.pod"))
    else {
        return;
    };
    let track = Track::from_archive(&archive).unwrap();
    let animated = &track.animated_models["OP88ANIM.BIN"];
    assert_eq!(animated.animation.frames.len(), 8);
    assert_eq!(animated.animation.frames[0], "op88_1.bin");
    assert_eq!(animated.animation.seconds_per_frame, 1.25);
    assert!(animated.frames.iter().all(|frame| frame.len() == 48));
    assert_eq!(track.models["OP88ANIM.BIN"].vertices, animated.frames[0]);
    // Its frames differ: it moves.
    assert_ne!(animated.frames[0], animated.frames[4]);
}
