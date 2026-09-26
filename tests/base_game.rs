//! Checks the base game's archives against real files, which can't be committed: they are
//! read from `base/` at the root of the repository (links to the `Shared` and `English`
//! folders of an MTM2 CD will do), with the tracks in `tracks/` and the trucks in `trucks/`.
//! The tests pass trivially when those are empty.

use std::path::{Path, PathBuf};

use monster_truck_rural_ruckus::base_game::{BaseGame, DEFAULT_FOLDER};
use monster_truck_rural_ruckus::pod::{
    self, BaseArchives, BoxShape, NoBase, PodArchive, Track, Truck,
};
use monster_truck_rural_ruckus::{track, truck};

fn base() -> Option<BaseGame> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_FOLDER);
    let base = BaseGame::open(&[folder]);
    (base.archive_count() > 0).then_some(base)
}

fn archives_in(folder: &str) -> Vec<(PathBuf, PodArchive)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(folder);
    let Ok(files) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = files
        .flatten()
        .map(|file| file.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pod"))
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let archive = PodArchive::parse(std::fs::read(&path).unwrap()).unwrap();
            (path, archive)
        })
        .collect()
}

/// What a track names and could not be found: models, backdrop models, the models'
/// textures, ground textures, and the palette.
fn missing_from_track(track: &Track) -> [usize; 5] {
    let mut named_models: Vec<String> = track
        .situation
        .boxes
        .iter()
        .filter_map(|situation_box| match &situation_box.shape {
            BoxShape::Model(name) => Some(name.to_ascii_uppercase()),
            BoxShape::Dimensions(_) => None,
        })
        .collect();
    named_models.sort();
    named_models.dedup();
    let models = named_models
        .iter()
        .filter(|name| !track.models.contains_key(*name))
        .count();
    let backdrops = track
        .situation
        .backdrops
        .iter()
        .filter(|name| {
            !track
                .backdrop_models
                .contains_key(&name.to_ascii_uppercase())
        })
        .count();
    let mut texture_names: Vec<String> = track
        .models
        .values()
        .chain(track.backdrop_models.values())
        .flat_map(|model| model.textures())
        .map(str::to_ascii_uppercase)
        .collect();
    texture_names.sort();
    texture_names.dedup();
    let textures = texture_names
        .iter()
        .filter(|name| {
            !track.model_textures.contains_key(*name)
                && !track.model_hd_textures.contains_key(*name)
        })
        .count();
    let ground = track
        .ground_textures
        .iter()
        .zip(&track.ground_hd_textures)
        .filter(|(texture, hd)| texture.is_none() && hd.is_none())
        .count();
    [
        models,
        backdrops,
        textures,
        ground,
        usize::from(track.palette.is_none()),
    ]
}

/// What a truck names and could not be found: parts (body, axle, a tire for each side),
/// and the textures of the parts it has.
fn missing_from_truck(truck: &Truck) -> [usize; 2] {
    let parts = [
        &truck.body,
        &truck.axle,
        &truck.left_tire,
        &truck.right_tire,
    ]
    .iter()
    .filter(|part| part.is_none())
    .count();
    let mut texture_names: Vec<String> = [
        &truck.body,
        &truck.axle,
        &truck.left_tire,
        &truck.right_tire,
    ]
    .into_iter()
    .chain(&truck.wheel_tires)
    .flatten()
    .flat_map(|model| model.textures())
    .chain(truck.file.bar_texture.as_deref())
    .chain(truck.file.shock_texture.as_deref())
    .map(str::to_ascii_uppercase)
    .collect();
    texture_names.sort();
    texture_names.dedup();
    let textures = texture_names
        .iter()
        .filter(|name| {
            !truck.textures.contains_key(*name) && !truck.hd_textures.contains_key(*name)
        })
        .count();
    [parts, textures]
}

#[test]
fn a_file_is_found_by_reading_only_the_archive_that_holds_it() {
    let Some(base) = base() else {
        return;
    };
    assert!(
        base.archives_read().is_empty(),
        "opening reads directories only"
    );

    // The palette that trucks fall back to. See docs/formats/truck.md.
    let archive = base
        .holding("ART", "metalcr2.act")
        .expect("METALCR2.ACT in the base game");
    assert!(archive.find_file_in("ART", "METALCR2.ACT").is_some());
    assert_eq!(base.archives_read().len(), 1);

    assert!(base.holding("ART", "NO_SUCH_FILE.RAW").is_none());
    assert_eq!(base.archives_read().len(), 1);
}

#[test]
fn the_base_game_never_leaves_more_missing() {
    let Some(base) = base() else {
        return;
    };
    for (path, archive) in archives_in("tracks") {
        let alone = missing_from_track(&Track::from_archives(&archive, &NoBase).unwrap());
        let with_base = missing_from_track(&Track::from_archives(&archive, &base).unwrap());
        for (alone, with_base) in alone.iter().zip(with_base) {
            assert!(
                with_base <= *alone,
                "{}: {alone:?} -> {with_base:?}",
                path.display()
            );
        }
    }
    for (path, archive) in archives_in("trucks") {
        let alone = missing_from_truck(&Truck::from_archives(&archive, &NoBase).unwrap());
        let with_base = missing_from_truck(&Truck::from_archives(&archive, &base).unwrap());
        for (alone, with_base) in alone.iter().zip(with_base) {
            assert!(
                with_base <= *alone,
                "{}: {alone:?} -> {with_base:?}",
                path.display()
            );
        }
    }
}

#[test]
fn every_track_and_truck_converts_with_what_it_borrows() {
    let Some(base) = base() else {
        return;
    };
    for (path, _) in archives_in("tracks") {
        track::load_pod(&path, &base).unwrap_or_else(|error| panic!("{error}"));
    }
    for (path, _) in archives_in("trucks") {
        truck::load_pod(&path, &base).unwrap_or_else(|error| panic!("{error}"));
    }
}

/// `cargo test --test base_game -- --ignored --nocapture` prints each of the base game's
/// tracks and trucks, and whether it loads.
#[test]
#[ignore = "prints figures, and loads every base track"]
fn print_the_base_games_tracks_and_trucks() {
    let Some(base) = base() else {
        return;
    };
    for found in track::peek_base(&base) {
        let loaded = track::load_base(&base, &found.archive, &found.file);
        let missing = missing_from_track(
            &Track::from_file(base.archive(&found.archive).unwrap(), &found.file, &base).unwrap(),
        );
        println!(
            "track {:<22} {:<16} {:?}  {}  missing {missing:?}",
            format!("{:?}", found.name),
            found.file,
            found.archive.file_name().unwrap(),
            loaded.map_or_else(
                |error| error,
                |track| format!("{} gates", track.gates.len())
            )
        );
    }
    for found in truck::peek_base(&base) {
        let loaded = truck::load_base(&base, &found.archive, &found.file);
        let missing = missing_from_truck(
            &Truck::from_file(base.archive(&found.archive).unwrap(), &found.file, &base).unwrap(),
        );
        println!(
            "truck {:<22} {:<16} {}  missing {missing:?}",
            format!("{:?}", found.name),
            found.file,
            loaded.map_or_else(|error| error, |truck| truck.name)
        );
    }
}

/// `cargo test --test base_game -- --ignored --nocapture` prints, for each track and truck,
/// what it names and cannot be found, alone and with the base game, and which base archives
/// each one had to read.
#[test]
#[ignore = "prints figures"]
fn print_what_the_base_game_supplies() {
    let Some(_) = base() else {
        return;
    };
    let name = |path: &Path| path.file_name().unwrap().to_string_lossy().into_owned();
    let read =
        |base: &BaseGame| -> Vec<String> { base.archives_read().into_iter().map(name).collect() };

    println!("track: models, backdrops, model textures, ground textures, palette");
    for (path, archive) in archives_in("tracks") {
        // A fresh base for each, so that it shows what that track alone reads.
        let base = base().unwrap();
        let alone = missing_from_track(&Track::from_archives(&archive, &NoBase).unwrap());
        let with_base = missing_from_track(&Track::from_archives(&archive, &base).unwrap());
        println!(
            "{:<28} {alone:?} -> {with_base:?}  reads {:?}",
            name(&path),
            read(&base)
        );
    }
    println!("truck: parts, textures");
    for (path, archive) in archives_in("trucks") {
        let base = base().unwrap();
        let alone = missing_from_truck(&Truck::from_archives(&archive, &NoBase).unwrap());
        let with_base = missing_from_truck(&Truck::from_archives(&archive, &base).unwrap());
        println!(
            "{:<28} {alone:?} -> {with_base:?}  reads {:?}",
            name(&path),
            read(&base)
        );
    }
}

/// Every sky's pixels are in the 16 slots its colours go into, whichever of the two
/// places those are (`pod::first_slot`): measured, every pixel of every sky of the user's
/// tracks and of the base game. And with the base game, every track has its own sky and
/// the base game's cloudy, dusk and night ones.
#[test]
fn every_sky_is_all_in_its_sixteen_slots() {
    let Some(base) = base() else {
        return;
    };
    for (path, archive) in archives_in("tracks") {
        let track = Track::from_archives(&archive, &base).unwrap();
        let skies = &track.skies;
        for (which, sky) in [
            ("own", &skies.own),
            ("cloudy", &skies.cloudy),
            ("dusk", &skies.dusk),
            ("night", &skies.night),
        ] {
            let sky = sky
                .as_ref()
                .unwrap_or_else(|| panic!("{}: no {which} sky", path.display()));
            let first = pod::first_slot(&sky.texture);
            let size = sky.texture.size();
            for y in 0..size {
                for x in 0..size {
                    let slot = sky.texture.index(x, y) as usize;
                    assert!(
                        (first..first + 16).contains(&slot),
                        "{}: {which} sky, pixel {x},{y} in slot {slot}, not {first} up",
                        path.display()
                    );
                }
            }
        }
    }
}

/// The base game's two placed models that move by keyframes: Crazy '98's dinosaur and
/// Tinhorn Junction's pump jacks. Each is an animation control file whose frames all have
/// as many vertices as the first, and each becomes a scenery model that moves.
#[test]
fn the_base_games_keyframed_models_load_with_all_their_frames() {
    let Some(base) = base() else {
        return;
    };
    // (control file, frames, vertices, rate read as 16.16)
    let expected = [("REX.BIN", 4, 822, 1.0), ("PUMPJACK.BIN", 8, 286, 0.5)];
    let mut found = Vec::new();
    for listed in track::peek_base(&base) {
        let archive = listed
            .archive
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_uppercase();
        if !["CRAZY98.POD", "OUTBACK.POD"].contains(&archive.as_str()) {
            continue;
        }
        let pod_archive = base.archive(&listed.archive).unwrap();
        let pod_track = Track::from_file(pod_archive, &listed.file, &base).unwrap();
        for (name, animated) in &pod_track.animated_models {
            let first = &animated.frames[0];
            assert_eq!(first, &pod_track.models[name].vertices, "{name}");
            assert!(
                animated
                    .frames
                    .iter()
                    .all(|frame| frame.len() == first.len()),
                "{name}"
            );
            found.push((
                name.clone(),
                animated.frames.len(),
                first.len(),
                animated.animation.seconds_per_frame,
            ));
        }

        let converted = track::load_base(&base, &listed.archive, &listed.file).unwrap();
        for model in &converted.scenery.models {
            let Some(keyframes) = &model.keyframes else {
                continue;
            };
            assert!(pod_track.animated_models.contains_key(&model.name));
            assert_eq!(keyframes.frames[0], model.positions, "{}", model.name);
            for frame in &keyframes.frames {
                assert_eq!(frame.len(), model.positions.len(), "{}", model.name);
            }
            let corners: u32 = keyframes.faces.iter().map(|face| face.corners).sum();
            assert_eq!(corners as usize, model.positions.len(), "{}", model.name);
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup_by(|a, b| a.0 == b.0);
    for (name, frames, vertices, seconds) in expected {
        assert!(
            found.contains(&(name.to_string(), frames, vertices, seconds)),
            "{name} in {found:?}"
        );
    }
}

/// Four trucks have an engine whose texture steps between two frames. Each converts to a
/// mesh that steps through them.
#[test]
fn trucks_with_an_animated_texture_convert_to_meshes_that_step_through_it() {
    let Some(base) = base() else {
        return;
    };
    for (path, archive) in archives_in("trucks") {
        let pod_truck = Truck::from_archives(&archive, &base).unwrap();
        let animated = pod_truck
            .body
            .as_ref()
            .is_some_and(|body| body.faces.iter().any(|face| face.texture_cycle.is_some()));
        let looks = truck::load_pod(&path, &base).unwrap().looks.unwrap();
        let cycles: Vec<_> = looks
            .body
            .iter()
            .flat_map(|body| &body.parts)
            .filter_map(|part| part.texture_cycle.as_ref())
            .collect();
        assert_eq!(animated, !cycles.is_empty(), "{}", path.display());
        for cycle in cycles {
            assert!(cycle.frames.len() >= 2 && cycle.seconds_per_frame > 0.0);
            assert!(
                cycle
                    .frames
                    .iter()
                    .all(|&frame| frame < looks.textures.len())
            );
        }
    }
}

/// Every truck lights the road at night: at least two lamps with a beam, whether its file
/// gives them (the base game's), gives headlights with none (Maximum Destruction), or gives
/// no lights at all (many community trucks), where the game adds its own.
#[test]
fn every_truck_has_headlights_that_light_the_road() {
    let Some(base) = base() else {
        return;
    };
    for (path, _) in archives_in("trucks") {
        let truck = truck::load_pod(&path, &base).unwrap_or_else(|error| panic!("{error}"));
        let beams = truck
            .lamps
            .iter()
            .filter(|lamp| lamp.beam.is_some())
            .count();
        assert!(beams >= 2, "{}: {beams} beams", path.display());
    }
}

/// Every truck names the base game's dashboard, `powerbig`, which is found in its
/// `COCKPIT.POD` at the largest screen, with its four backgrounds and a steering wheel
/// turned 5 to 35 each way. See `docs/formats/cockpit.md`.
#[test]
fn every_base_truck_finds_the_dashboard_it_names() {
    let Some(base) = base() else {
        return;
    };
    let mut trucks = 0;
    for found in truck::peek_base(&base) {
        let archive = base.archive(&found.archive).unwrap();
        let loaded = Truck::from_file(archive, &found.file, &base).unwrap();
        let name = &found.file;
        assert_eq!(loaded.file.instrument_cluster.as_deref(), Some("powerbig"));
        let cockpit = loaded
            .cockpit
            .unwrap_or_else(|| panic!("{name}: no cockpit"));
        assert_eq!(cockpit.file, "POWERBIG.480");
        assert_eq!(cockpit.screen, [640, 480]);
        assert_eq!(cockpit.backgrounds.len(), 4);
        assert!(cockpit.backgrounds.iter().all(Option::is_some), "{name}");
        let turns: Vec<i32> = cockpit
            .steering_wheel
            .iter()
            .map(|(turn, _)| *turn)
            .collect();
        assert_eq!(turns, (-35..=35).step_by(5).collect::<Vec<_>>());
        trucks += 1;
    }
    assert!(trucks >= 20, "{trucks} trucks");
}

/// The dashboard as the base game has it, read through any truck, with its layout.
fn powerbig() -> Option<pod::Cockpit> {
    let base = base()?;
    let found = truck::peek_base(&base).into_iter().next()?;
    let archive = base.archive(&found.archive)?;
    Truck::from_file(archive, &found.file, &base).ok()?.cockpit
}

/// Index 0 is where the 3D view shows through: in each background it is the only black in
/// the palette, every pixel of it is inside the layout's 3D window, and it fills most of
/// that window. Round the steering wheel it is the corners of its picture, and not its hub.
#[test]
fn the_3d_view_shows_through_palette_index_0() {
    let Some(cockpit) = powerbig() else {
        return;
    };
    let [left, top, width, height] = cockpit.layout.window.unwrap().map(|value| value as usize);
    assert_eq!([left, top, width, height], [0, 88, 640, 240]);
    for background in cockpit.backgrounds.iter().flatten() {
        let picture = &background.picture;
        let blacks: Vec<u8> = (0..=255u8)
            .filter(|&index| background.palette.color(index) == [0, 0, 0])
            .collect();
        assert_eq!(blacks, [pod::SEE_THROUGH_INDEX]);
        let mut inside = 0;
        for y in 0..picture.height() {
            for x in 0..picture.width() {
                if picture.index(x, y) != pod::SEE_THROUGH_INDEX {
                    continue;
                }
                assert!(
                    (left..left + width).contains(&x) && (top..top + height).contains(&y),
                    "see-through pixel at {x}, {y}"
                );
                inside += 1;
            }
        }
        assert!(inside * 3 > width * height, "{inside} see-through pixels");
    }
    for (turn, frame) in &cockpit.steering_wheel {
        let picture = &frame.picture;
        let (right, bottom) = (picture.width() - 1, picture.height() - 1);
        for (x, y) in [(0, 0), (right, 0)] {
            assert_eq!(picture.index(x, y), pod::SEE_THROUGH_INDEX, "{turn}");
        }
        let hub = picture.index(picture.width() / 2, bottom);
        assert_ne!(hub, pod::SEE_THROUGH_INDEX, "{turn}");
    }
}

/// How bright a picture is at a point, from 0 to 255, or 0 off the picture.
fn brightness(picture: &pod::CockpitPicture, x: f32, y: f32) -> f32 {
    let (x, y) = (x.round(), y.round());
    let (width, height) = (picture.picture.width(), picture.picture.height());
    if x < 0.0 || y < 0.0 || x as usize >= width || y as usize >= height {
        return 0.0;
    }
    let [red, green, blue] = picture
        .palette
        .color(picture.picture.index(x as usize, y as usize));
    (red as f32 + green as f32 + blue as f32) / 3.0
}

/// How many bright pixels a dial has just beyond its needle's tip, along the best of the
/// lines within 3 degrees of `degrees` clockwise from straight up.
fn mark_at(picture: &pod::CockpitPicture, gauge: &pod::Gauge, degrees: f32) -> usize {
    let [x, y] = gauge.center.unwrap();
    let radius = gauge.radius.unwrap();
    (-6..=6)
        .map(|step| {
            let angle = (degrees + step as f32 * 0.5).to_radians();
            (0..=20)
                .filter(|&step| {
                    let out = radius + 3.0 + step as f32 * 0.25;
                    brightness(picture, x + out * angle.sin(), y - out * angle.cos()) > 70.0
                })
                .count()
        })
        .max()
        .unwrap_or(0)
}

/// The needle points `zero angle + degrees per unit x value`, in degrees clockwise from
/// straight up on the screen: that puts it on the dial's printed marks, 10 to 100 mph and
/// 1,000 to 9,000 rpm, just beyond the needle's tip. Counted the other way round it misses
/// most of them.
#[test]
fn the_dials_turn_clockwise_from_straight_up() {
    let Some(cockpit) = powerbig() else {
        return;
    };
    let ahead = cockpit.backgrounds[0].as_ref().unwrap();
    let dials = [
        (&cockpit.layout.speedometer, (10..=100).step_by(10)),
        (&cockpit.layout.tachometer, (1000..=9000).step_by(1000)),
    ];
    for (gauge, marks) in dials {
        let zero = gauge.zero_angle.unwrap();
        let rate = gauge.degrees_per_unit.unwrap();
        let mut missed_the_other_way = 0;
        for mark in marks {
            let angle = zero + rate * mark as f32;
            let found = mark_at(ahead, gauge, angle);
            assert!(found >= 12, "mark {mark} at {angle} degrees: {found}");
            if mark_at(ahead, gauge, -angle) < 6 {
                missed_the_other_way += 1;
            }
        }
        assert!(missed_the_other_way >= 4, "{missed_the_other_way}");
    }
}

/// A base truck's dashboard converts with a picture for every way of looking, its dials
/// turning as the layout says, and its steering wheel from full right to full left. The
/// built-in truck has none.
#[test]
fn a_base_truck_converts_with_its_dashboard() {
    assert!(truck::TruckData::builtin().dashboard.is_none());
    let Some(base) = base() else {
        return;
    };
    let Some(found) = truck::peek_base(&base).into_iter().next() else {
        return;
    };
    let converted = truck::load_base(&base, &found.archive, &found.file).unwrap();
    let dashboard = converted.dashboard.expect("a dashboard");
    assert_eq!(dashboard.screen.to_array(), [640.0, 480.0]);
    assert!(dashboard.views.iter().all(Option::is_some));
    let ahead = dashboard.views[0].as_ref().unwrap();
    assert_eq!((ahead.width, ahead.height), (640, 480));
    // The middle of the 3D window is see-through, the dials are not.
    let alpha = |x: usize, y: usize| ahead.rgba[(y * 640 + x) * 4 + 3];
    assert_eq!(alpha(320, 200), 0);
    assert_eq!(alpha(153, 300), 255);
    let speedometer = dashboard.speedometer.unwrap();
    assert!((speedometer.zero.to_degrees() - 304.5).abs() < 1e-3);
    // 2.65 degrees per mph.
    let per_mph = speedometer.per_unit.to_degrees() * 1609.344 / 3600.0;
    assert!((per_mph - 2.65).abs() < 1e-4, "{per_mph}");
    let wheel = dashboard.steering_wheel.as_ref().unwrap();
    assert_eq!(wheel.frames.len(), 15);
    assert_eq!(wheel.frames.first().unwrap().0, -1.0);
    assert_eq!(wheel.frames.last().unwrap().0, 1.0);
}
