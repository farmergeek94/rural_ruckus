//! Checks the truck parsers against real archives, which can't be committed: they are
//! read from `trucks/` at the root of the repository, and the tests pass trivially when
//! there are none.

use std::collections::BTreeMap;
use std::path::Path;

use monster_truck_rural_ruckus::pod::{self, Model, PodArchive, Truck};

fn real_archives() -> Vec<(String, PodArchive)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("trucks");
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

fn truck(file_name: &str) -> Option<(PodArchive, Truck)> {
    let (_, archive) = real_archives()
        .into_iter()
        .find(|(path, _)| path.ends_with(file_name))?;
    let truck = Truck::from_archive(&archive).unwrap();
    Some((archive, truck))
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The normal that the order of a face's corners gives by the right-hand rule, summed
/// over every edge because the first three corners of some faces lie in a line.
fn winding_normal(model: &Model, face: &pod::Face) -> [f32; 3] {
    let mut normal = [0.0; 3];
    for (index, corner) in face.corners.iter().enumerate() {
        let a = model.vertices[corner.vertex];
        let b = model.vertices[face.corners[(index + 1) % face.corners.len()].vertex];
        normal[0] += a[1] * b[2] - a[2] * b[1];
        normal[1] += a[2] * b[0] - a[0] * b[2];
        normal[2] += a[0] * b[1] - a[1] * b[0];
    }
    normal
}

/// A list of trucks reads the name alone, and must call a truck what loading it does.
#[test]
fn every_real_truck_gives_its_name_without_being_loaded() {
    for (path, archive) in real_archives() {
        let name = Truck::name_in(&archive)
            .unwrap_or_else(|| panic!("{path}: no truck"))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert!(!name.is_empty(), "{path}");
        assert_eq!(name, Truck::from_archive(&archive).unwrap().file.name);
        assert!(pod::Track::name_in(&archive).is_none(), "{path}");
    }
}

/// Truck models have a record that no track model has, a normal for each vertex, and
/// the parser used to stop at it. Not every model has it.
#[test]
fn every_real_truck_model_reads_to_its_end() {
    for (path, archive) in real_archives() {
        assert!(Truck::is_in(&archive), "{path}");
        let mut kinds: BTreeMap<i32, usize> = BTreeMap::new();
        for entry in archive.entries() {
            if !entry.extension().eq_ignore_ascii_case("bin") {
                continue;
            }
            let model = Model::parse(archive.data(entry))
                .unwrap_or_else(|error| panic!("{}: {error}", entry.name));
            assert_eq!(model.incomplete, None, "{}", entry.name);
            assert!(!model.faces.is_empty(), "{}", entry.name);
            // A model may bring no normals at all (Max-D's drive shaft), and otherwise
            // one for each vertex.
            assert!(
                model.vertex_normals.is_empty()
                    || model.vertex_normals.len() == model.vertices.len(),
                "{}",
                entry.name
            );
            for face in &model.faces {
                *kinds.entry(face.kind).or_default() += 1;
            }
        }
        println!("{path}: faces by type {kinds:?}");
    }
}

/// Facts read out of this particular archive by hand.
#[test]
fn bigfoot_matches_what_is_in_the_file() {
    let Some((archive, truck)) = truck("99BFoot.pod") else {
        return;
    };
    assert_eq!(truck.file.name, "Bigfoot 1999 Superduty");
    assert_eq!(truck.file.body_model, "bf99_1.bin");
    assert_eq!(truck.file.tire_model_base, "BF99_");
    assert_eq!(truck.file.tires.front_right, [4.5, -3.65, 6.61]);
    assert_eq!(truck.file.tires.rear_left, [-4.5, -3.65, -6.38]);
    assert_eq!(truck.file.scrape_points.len(), 12);
    assert_eq!(truck.file.scrape_points[4], [-2.289062, 5.050781, 2.707031]);

    // Every part is in the archive, and every texture they use, each with a palette of
    // its own. Textures are 64 or 256 pixels a side.
    let parts = [
        &truck.body,
        &truck.left_tire,
        &truck.right_tire,
        &truck.axle,
    ];
    for part in parts {
        for name in part.as_ref().expect("a part is missing").textures() {
            let texture = &truck.textures[&name.to_ascii_uppercase()].texture;
            assert!([64, 256].contains(&texture.size()), "{name}");
        }
    }
    assert!(archive.find_file("bf99_1.act").is_some());
}

/// A truck's parts are borrowed from the base game as often as not.
#[test]
fn virginia_giant_loads_without_its_tires_and_axle() {
    let Some((_, truck)) = truck("VirginiaGiant2003.pod") else {
        return;
    };
    assert_eq!(truck.file.name, "Virginia Giant");
    assert_eq!(truck.file.tire_model_base, "sil");
    assert!(truck.body.is_some());
    assert!(truck.left_tire.is_none() && truck.right_tire.is_none() && truck.axle.is_none());
    assert_eq!(truck.file.scrape_points.len(), 12);
}

/// A Community Patch 3 truck: an MTM2.1 truck file, a tire model for each wheel, PNG
/// textures, and a body whose glass is drawn with material records (63) and faces of
/// type 64.
#[test]
fn gmc_is_an_mtm2_1_truck_with_png_textures() {
    let Some((_, truck)) = truck("GMC1500TT.POD") else {
        return;
    };
    assert_eq!(truck.file.name, "GMC 1500 Trophy Truck Evo2");
    assert!(truck.file.extended);
    assert_eq!(truck.file.tire_model_base, "bfsw");

    let body = truck.body.as_ref().unwrap();
    assert_eq!(body.incomplete, None);
    assert_eq!(body.faces.len(), 2706);
    let glass: Vec<_> = body.faces.iter().filter(|face| face.kind == 64).collect();
    assert_eq!(glass.len(), 30);
    assert_eq!(glass.iter().filter(|face| face.is_cutout()).count(), 14);
    // Flags 0x16b (alpha test) and 0x167 (blend), neither of them two-sided.
    assert!(glass.iter().all(|face| !face.is_two_sided()));

    // The rear tires are models of their own. The front ones are the same as the side's.
    let tires = truck
        .wheel_tires
        .each_ref()
        .map(|tire| tire.as_ref().unwrap());
    assert_eq!(tires[0], truck.left_tire.as_ref().unwrap());
    assert_eq!(tires[1], truck.right_tire.as_ref().unwrap());
    assert_ne!(tires[2], tires[0]);
    assert_ne!(tires[3], tires[1]);

    // Textures with a PNG have no .RAW, and the rest are 8-bit as before.
    let sizes: BTreeMap<_, _> = truck
        .hd_textures
        .iter()
        .map(|(name, texture)| (name.as_str(), texture.size()))
        .collect();
    assert_eq!(
        sizes,
        BTreeMap::from([
            ("ARBMONSTER.RAW", 512),
            ("BFSSW1.RAW", 256),
            ("GD-W1.RAW", 256),
            ("GMC1500TPY.RAW", 512),
        ])
    );
    assert!(sizes.keys().all(|name| !truck.textures.contains_key(*name)));
    // Four for the models, the shocks' `shockss.raw`, and its brake lights' `braklite.raw`.
    // The axle bars' `axlebar2.raw` is the base game's, as is the beams' `redfuzz.raw`.
    assert_eq!(truck.textures.len(), 6);
    assert!(truck.textures.contains_key("BRAKLITE.RAW"));
    assert!(truck.textures.contains_key("SHOCKSS.RAW"));
    assert!(!truck.textures.contains_key("AXLEBAR2.RAW"));
}

/// What the axes of a truck file mean. "rtire" and "ltire" say which side X is, "faxle"
/// and "raxle" which end Z is, and the body model, which reaches below the scrape points
/// only by its chassis, has to fit between them.
#[test]
fn bigfoot_axes_are_right_up_and_forwards() {
    let Some((_, truck)) = truck("99BFoot.pod") else {
        return;
    };
    let tires = &truck.file.tires;
    assert!(tires.front_right[0] > 0.0 && tires.rear_right[0] > 0.0);
    assert!(tires.front_left[0] < 0.0 && tires.rear_left[0] < 0.0);
    assert!(tires.front_left[2] > 0.0 && tires.rear_left[2] < 0.0);

    // The body is in the same axes and from the same origin: it spans the scrape points
    // to within a foot or so at each end, and the tires hang below it.
    let (low, high) = truck.body.as_ref().unwrap().bounds();
    let points = &truck.file.scrape_points;
    let reach = |axis: usize| {
        let values = points.iter().map(|point| point[axis]);
        (
            values.clone().fold(f32::INFINITY, f32::min),
            values.fold(f32::NEG_INFINITY, f32::max),
        )
    };
    for axis in [0, 2] {
        let (from, to) = reach(axis);
        assert!((low[axis] - from).abs() < 1.5, "axis {axis}: {low:?}");
        assert!((high[axis] - to).abs() < 1.5, "axis {axis}: {high:?}");
    }
    assert!((high[1] - reach(1).1).abs() < 0.5);
    assert!(tires.front_left[1] < reach(1).0);
}

/// A tire is a wheel centred on its own origin with its axle along X, so its radius is
/// how far it reaches up. The left and right tires are mirror images.
#[test]
fn bigfoot_tires_are_centred_wheels_six_feet_across() {
    let Some((_, truck)) = truck("99BFoot.pod") else {
        return;
    };
    let (left, right) = (truck.left_tire.unwrap(), truck.right_tire.unwrap());
    let (low, high) = left.bounds();
    assert_eq!((low[1], high[1], low[2], high[2]), (-3.0, 3.0, -3.0, 3.0));
    assert!(high[0] - low[0] < 4.5);
    let (right_low, right_high) = right.bounds();
    assert_eq!((right_low[0], right_high[0]), (-high[0], -low[0]));
}

/// The record this is about is only known from these files. If it holds normals, they
/// are of unit length, they point away from the middle of a tire, and they agree with
/// the faces around them about which side is out. On the tire, every one does. The body,
/// made with a community tool, has some that are zero and some on the wrong side, so
/// whoever shades with them has to check each against its face.
#[test]
fn bigfoot_vertex_normals_point_outwards() {
    let Some((_, truck)) = truck("99BFoot.pod") else {
        return;
    };
    let tire = truck.left_tire.unwrap();
    let outwards = tire
        .vertices
        .iter()
        .zip(&tire.vertex_normals)
        .filter(|(vertex, normal)| dot(**vertex, **normal) > 0.0)
        .count();
    println!(
        "tire: {outwards} of {} normals point away from the hub",
        tire.vertices.len()
    );
    // Not all of them: the wheel in the middle of the tire is dished inwards.
    assert!(outwards * 10 >= tire.vertices.len() * 8);

    let body = truck.body.as_ref().unwrap();
    for (name, model, percent) in [("tire", &tire, 100), ("body", body, 80)] {
        let (mut unit, mut used) = (0, 0);
        let (mut agree, mut corners) = (0, 0);
        let mut is_used = vec![false; model.vertices.len()];
        for face in &model.faces {
            let face_normal = winding_normal(model, face);
            for corner in &face.corners {
                is_used[corner.vertex] = true;
                corners += 1;
                if dot(face_normal, model.vertex_normals[corner.vertex]) > 0.0 {
                    agree += 1;
                }
            }
        }
        for (index, normal) in model.vertex_normals.iter().enumerate() {
            if is_used[index] {
                used += 1;
                if (dot(*normal, *normal).sqrt() - 1.0).abs() < 0.001 {
                    unit += 1;
                }
            }
        }
        println!(
            "{name}: {unit} of {used} used normals are unit; {agree} of {corners} corners agree with their face's winding"
        );
        assert!(unit * 100 >= used * percent.min(90), "{name}");
        assert!(agree * 100 >= corners * percent, "{name}");
    }
}

/// Where an axle goes. The ends of its tube, which meet the wheels, are centred on its
/// own origin in height, so the origin belongs on the line between the two hubs. (Centred
/// on its bounds instead, the tube would hang most of a foot below them.)
#[test]
fn bigfoot_axle_tube_is_centred_on_its_origin() {
    let Some((_, truck)) = truck("99BFoot.pod") else {
        return;
    };
    let axle = truck.axle.unwrap();
    let (low, high) = axle.bounds();
    assert!(
        (low[1] + high[1]).abs() / 2.0 > 0.9,
        "the bounds are not centred"
    );
    for side in [-1.0, 1.0] {
        let end: Vec<_> = axle
            .vertices
            .iter()
            .filter(|vertex| vertex[0] * side > 3.0)
            .collect();
        assert!(end.len() >= 4);
        let top = end.iter().map(|vertex| vertex[1]).fold(f32::MIN, f32::max);
        let bottom = end.iter().map(|vertex| vertex[1]).fold(f32::MAX, f32::min);
        assert!((top + bottom).abs() < 0.01, "{bottom} to {top}");
    }
    // And it reaches most of the way to the tires, 4.5 ft out.
    assert!(high[0] > 3.0 && low[0] < -3.0);
}

/// Handedness, from lettering. The body's paintwork says "Bigfoot" on both sides, and
/// lettering reads from left to right seen from outside. Standing on the truck's right
/// its front is to your right, so on faces that look right the texture must run towards
/// the front, and on faces that look left, towards the back. With "right" being +X (the
/// truck file's `rtire`) and "front" +Z (`faxle`), and Y up, that makes the file's axes
/// left-handed: showing them in right-handed axes needs one axis flipped, or every truck
/// says "toofgiB".
#[test]
fn bigfoot_lettering_reads_forwards_in_left_handed_axes() {
    let Some((_, truck)) = truck("99BFoot.pod") else {
        return;
    };
    let body = truck.body.unwrap();
    let (mut reads_forwards, mut sides) = (0, 0);
    for face in &body.faces {
        if !face
            .texture
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case("BF99_1.RAW"))
        {
            continue;
        }
        let normal = winding_normal(&body, face);
        let length = dot(normal, normal).sqrt();
        if length == 0.0 || normal[0].abs() / length < 0.9 {
            continue;
        }
        // How the texture's "across" changes along the truck, over the face's corners.
        let count = face.corners.len() as f32;
        let mean_u = face.corners.iter().map(|corner| corner.uv[0]).sum::<f32>() / count;
        let mean_z = face
            .corners
            .iter()
            .map(|corner| body.vertices[corner.vertex][2])
            .sum::<f32>()
            / count;
        let across_per_forwards: f32 = face
            .corners
            .iter()
            .map(|corner| (corner.uv[0] - mean_u) * (body.vertices[corner.vertex][2] - mean_z))
            .sum();
        if across_per_forwards == 0.0 {
            continue;
        }
        sides += 1;
        if (across_per_forwards > 0.0) == (normal[0] > 0.0) {
            reads_forwards += 1;
        }
    }
    println!("{reads_forwards} of {sides} side faces read forwards");
    assert!(sides >= 20 && reads_forwards * 10 >= sides * 9);
}

/// Every truck file in `trucks/` and in the base game's archives in `base/`.
fn real_truck_files() -> Vec<(String, pod::TruckFile, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut archives: Vec<std::path::PathBuf> = Vec::new();
    for folder in ["trucks", "base/Shared", "base/English"] {
        let Ok(files) = std::fs::read_dir(root.join(folder)) else {
            continue;
        };
        archives.extend(files.flatten().map(|file| file.path()).filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pod"))
        }));
    }
    let mut found = Vec::new();
    for path in archives {
        let Ok(archive) = PodArchive::parse(std::fs::read(&path).unwrap()) else {
            continue;
        };
        for entry in Truck::files_in(archive.entries()) {
            let text = String::from_utf8_lossy(archive.data(entry));
            if let Ok(file) = pod::TruckFile::parse(&text) {
                let name = format!("{}: {}", path.display(), entry.name);
                found.push((name, file, text.into_owned()));
            }
        }
    }
    found
}

/// Which way round a light's heading goes. A lamp to one side of the middle that is turned
/// at all is turned outwards, to that side, not in across the truck: a heading turns from
/// forwards (+Z) towards the right (+X), so the way it shines is (sin heading, cos
/// heading) across and forwards. Measured on the 33 truck files of the user's and the base
/// game's archives: 121 lamps turn outwards under that rule and 5 in; under the mirror
/// image, 5 and 121. Beacons that go round are left out.
#[test]
fn a_light_turned_aside_turns_outwards() {
    let (mut outwards, mut inwards) = (0, 0);
    for (_, file, _) in real_truck_files() {
        for light in &file.lights {
            let across = light.heading.sin();
            if light.spin != 0.0 || light.position[0].abs() < 0.3 || across.abs() < 0.01 {
                continue;
            }
            if (across > 0.0) == (light.position[0] > 0.0) {
                outwards += 1;
            } else {
                inwards += 1;
            }
        }
    }
    assert!(outwards >= 10 * inwards, "{outwards} out, {inwards} in");
}

/// Every light that a truck file counts is read: none is left out as unreadable.
#[test]
fn every_real_truck_file_has_its_lights_read() {
    for (name, file, text) in real_truck_files() {
        let mut lines = text.lines().map(str::trim);
        let counted = lines
            .by_ref()
            .find(|line| *line == "Number of Lights")
            .and_then(|_| lines.next()?.parse().ok())
            .unwrap_or(0);
        assert_eq!(file.lights.len(), counted, "{name}");
        for light in &file.lights {
            assert!(light.bitmap_radius >= 0.0, "{name}");
            assert!(light.cone_length >= 0.0, "{name}");
            assert!(!light.bitmap.is_empty(), "{name}");
        }
    }
}

/// Captain USA, CARO4-1, Stabilizer and 2011 Rock Star have an engine whose texture steps
/// between two 256 x 256 frames, `AENGANI1.RAW` and `AENGANI2.RAW`, at rate 2729. Every frame of every animated texture of a truck is in its archive.
#[test]
fn every_animated_texture_of_a_truck_has_its_frames() {
    for (path, archive) in real_archives() {
        let truck = Truck::from_archive(&archive).unwrap();
        let Some(body) = &truck.body else {
            continue;
        };
        for cycle in &body.texture_cycles {
            println!(
                "{path}: {:?} at {} s",
                cycle.frames, cycle.seconds_per_frame
            );
            assert!(
                cycle.frames.len() >= 2 && cycle.seconds_per_frame > 0.0,
                "{path}"
            );
            for frame in &cycle.frames {
                let key = frame.to_ascii_uppercase();
                assert!(
                    truck.textures.contains_key(&key) || truck.hd_textures.contains_key(&key),
                    "{path}: {frame}"
                );
            }
        }
        let named = |name: &str| path.to_ascii_uppercase().ends_with(name);
        if [
            "CAPTN_USA.POD",
            "CARO4-1.POD",
            "STABILIZER.POD",
            "_2011ROCKSTAR.POD",
        ]
        .iter()
        .any(|name| named(name))
        {
            let cycle = &body.texture_cycles[0];
            let frames: Vec<String> = cycle
                .frames
                .iter()
                .map(|f| f.to_ascii_uppercase())
                .collect();
            assert_eq!(frames, ["AENGANI1.RAW", "AENGANI2.RAW"], "{path}");
            assert_eq!(cycle.seconds_per_frame, 2729.0 / 65536.0, "{path}");
            assert_eq!(truck.textures["AENGANI1.RAW"].texture.size(), 256, "{path}");
        }
    }
}
