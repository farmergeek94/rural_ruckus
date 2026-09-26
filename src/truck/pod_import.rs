//! Turns a Monster Truck Madness 2 truck, as the `pod` crate reads it, into `TruckData`.
//!
//! This is the one place where a truck's conventions are converted to the game's, as
//! `track/pod_import.rs` is for a track's, and the reasons for each rule are in
//! `docs/formats/truck.md`:
//!
//! - MTM2 works in feet, with X to the truck's right, Y up and Z forwards, which is
//!   left-handed. The game works in metres and its trucks face -Z, so Z is flipped.
//!   Flipping an axis turns every face inside out, so the corners of each face are taken
//!   in the opposite order.
//! - A truck file says where the parts go and nothing about how the truck drives, so the
//!   handling is the built-in truck's.
//! - The dashboard keeps the pixels of the screen its layout is made for; its dials turn
//!   in radians, per metre a second, and its see-through palette index becomes alpha 0
//!   (`docs/formats/cockpit.md`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;

use crate::base_game::BaseGame;
use crate::hd_texture;
use crate::pod::{self, PodArchive};

use super::data::{headlight_beam, headlight_facing, lamps_for_body};
use super::{
    AxleLink, AxleLinks, Beam, Dashboard, DashboardPicture, Dial, NormalMap, SteeringWheel,
    TruckConfig, TruckData, TruckLamp, TruckLooks, TruckMesh, TruckModel, TruckTexture,
    TruckTextureCycle,
};

const METRES_PER_FOOT: f32 = 0.3048;
/// Miles an hour in a metre a second.
const MPH_PER_METRE_PER_SECOND: f32 = 3600.0 / 1609.344;

/// The axle bars, shocks and driveshaft as JSTruckViewer rebuilds them
/// (`src/worker/truck-worker.js`, `src/viewer-scene.js`). The game draws them itself, from
/// the truck file's offsets and from numbers that no file holds; these are the viewer's,
/// which is **reference** for how they look, not for the game's own numbers. In feet, in
/// the truck file's axes.
///
/// How far out from the middle of its axle a bar meets it, either side.
const BAR_AXLE_SIDE_FEET: f32 = 535.0 / 256.0;
/// How far below the middle of its axle a bar meets it.
const BAR_AXLE_BELOW_FEET: f32 = 80.0 / 256.0;
/// How far from the middle of its axle a bar meets it, towards the other axle.
const BAR_AXLE_INWARD_FEET: f32 = 83.0 / 256.0;
/// How much higher than `axlebarOffset` a bar meets the body.
const BAR_BODY_ABOVE_FEET: f32 = 45.0 / 256.0;
/// The units of `superiorAxlebarOffset`, per foot.
const UPPER_BAR_UNITS_PER_FOOT: f32 = 256.0;
/// How far out from the middle of its axle a wheel's shocks stand, either side.
const SHOCK_SIDE_FEET: f32 = 542.0 / 256.0;
/// How far above the middle of its axle a shock meets it. It meets the body at the body's
/// origin height.
const SHOCK_AXLE_ABOVE_FEET: f32 = 85.0 / 256.0;
/// How far in front of the axle's middle, and behind it, each of a wheel's two shocks is.
const SHOCK_PAIR_APART_FEET: f32 = 70.0 / 256.0;
/// Radii, in feet.
const BAR_RADIUS_FEET: f32 = 0.10;
const SHOCK_RADIUS_FEET: f32 = 0.12;
const DRIVESHAFT_RADIUS_FEET: f32 = 0.14;
/// Colours where a part has no texture in the archive.
const BAR_GREY: [u8; 3] = [0xb6, 0xb6, 0xb6];
const SHOCK_GREY: [u8; 3] = [0xb7, 0xb7, 0xb7];
/// The driveshaft has no texture of its own in any file.
const DRIVESHAFT_BROWN: [u8; 3] = [0x4b, 0x32, 0x1f];

/// An untextured face's colour when its texture isn't in the archive.
const MISSING_TEXTURE_GREY: [u8; 3] = [128, 128, 128];

/// Whether the archive on disk holds a truck, rather than a track.
pub fn pod_holds_truck(path: &Path) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| PodArchive::parse(bytes).ok())
        .is_some_and(|archive| pod::Truck::is_in(&archive))
}

/// The name of the truck in an archive on disk, for a list of trucks: the archive's
/// directory and the first lines of its truck file, and none of its models or textures.
/// `Ok(None)` for an archive with no truck in it, and an error for one that can't be read.
pub fn peek_pod(path: &Path) -> Result<Option<String>, String> {
    let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|error| describe(&error))?;
    let archive = PodArchive::parse(bytes).map_err(|error| describe(&error))?;
    pod::Truck::name_in(&archive)
        .transpose()
        .map_err(|error| describe(&error))
}

/// A truck in one of the base game's archives.
pub struct BaseTruck {
    /// The archive it is in.
    pub archive: PathBuf,
    /// Its truck file in that archive, such as `TRUCK\\BIGFOOT.TRK`.
    pub file: String,
    /// The name it gives itself, or why its truck file can't be read.
    pub name: Result<String, String>,
}

/// Every truck in the base game's archives, for a list of trucks: each truck file is
/// read on its own, and nothing else of its archive.
pub fn peek_base(base: &BaseGame) -> Vec<BaseTruck> {
    let mut found = Vec::new();
    for (archive, entries) in base.directories() {
        for entry in pod::Truck::files_in(entries) {
            let name = base
                .read_one(archive, entry)
                .and_then(|bytes| pod::Truck::name_from(&bytes).map_err(|error| error.to_string()))
                .map_err(|error| format!("{}: {error}", entry.name));
            found.push(BaseTruck {
                archive: archive.to_path_buf(),
                file: entry.name.clone(),
                name,
            });
        }
    }
    found
}

/// Reads one truck of the base game's archives, as [`peek_base`] lists it.
pub fn load_base(base: &BaseGame, archive: &Path, file: &str) -> Result<TruckData, String> {
    let describe = |error: &dyn std::fmt::Display| format!("{} {file}: {error}", archive.display());
    let pod_archive = base
        .archive(archive)
        .ok_or_else(|| describe(&"is not a base archive that can be read"))?;
    let truck = pod::Truck::from_file(pod_archive, file, base).map_err(|error| describe(&error))?;
    Ok(truck_from_pod(&truck))
}

/// Reads the first truck in a POD archive on disk, with what it borrows from `base`.
pub fn load_pod(path: &Path, base: &BaseGame) -> Result<TruckData, String> {
    let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|error| describe(&error))?;
    let archive = PodArchive::parse(bytes).map_err(|error| describe(&error))?;
    let truck = pod::Truck::from_archives(&archive, base).map_err(|error| describe(&error))?;
    Ok(truck_from_pod(&truck))
}

/// How far below where its file draws them a truck's body rides on its hubs, in metres.
pub const BODY_DROP: f32 = 0.25;

pub fn truck_from_pod(truck: &pod::Truck) -> TruckData {
    let mut config = TruckConfig::default();

    // No field gives the size of a tire. Its model is a wheel centred on its origin, with
    // its axle along X. A model made for one wheel is the one drawn there.
    let tires = truck
        .wheel_tires
        .iter()
        .chain([&truck.left_tire, &truck.right_tire]);
    if let Some(tire) = tires.flatten().next() {
        let (low, high) = tire.bounds();
        config.wheel_radius = low[1].abs().max(high[1]) * METRES_PER_FOOT;
        config.wheel_width = (high[0] - low[0]) * METRES_PER_FOOT;
    }

    // The file says where the tires are with the truck at rest, which is just what the
    // suspension wants to know, so the truck stands the way it was drawn.
    let tires = &truck.file.tires;
    let at_rest = [
        tires.front_left,
        tires.front_right,
        tires.rear_left,
        tires.rear_right,
    ]
    .map(to_game);
    // The body sits lower on its axles than drawn, so that the ride, with
    // `suspension_bump` of travel above rest, comes out right.
    config.wheel_rest = at_rest.map(|hub| hub + Vec3::Y * BODY_DROP);

    // A monster truck's weight is in its axles and tires. The centre of mass stays as far
    // above the axles as the built-in truck's, so a taller body doesn't make a truck that
    // rolls over more readily than the one the handling was tuned on.
    let builtin = TruckConfig::default();
    let above_axles = builtin.center_of_mass.y - builtin.wheel_rest[0].y;
    let between_axles = config.wheel_rest.iter().sum::<Vec3>() / config.wheel_rest.len() as f32;
    config.center_of_mass = between_axles + Vec3::Y * above_axles;

    let mut textures = Textures {
        truck,
        made: Vec::new(),
        known: HashMap::new(),
    };
    let mut convert = |model: &Option<pod::Model>| {
        model
            .as_ref()
            .map(|model| convert_model(model, &mut textures))
    };
    let (body, left_tire, right_tire, axle) = (
        convert(&truck.body),
        convert(&truck.left_tire),
        convert(&truck.right_tire),
        convert(&truck.axle),
    );
    let wheel_tires = truck.wheel_tires.each_ref().map(convert);
    let mut texture = |name: &Option<String>| {
        name.as_deref()
            .and_then(|name| textures.index(name, Alpha::Ignored))
    };
    let file = &truck.file;
    let axle_bars = AxleLinks {
        links: axle_bars(file),
        radius: BAR_RADIUS_FEET * METRES_PER_FOOT,
        texture: texture(&file.bar_texture),
        color: BAR_GREY,
    };
    let shocks = AxleLinks {
        links: shocks(file),
        radius: SHOCK_RADIUS_FEET * METRES_PER_FOOT,
        texture: texture(&file.shock_texture),
        color: SHOCK_GREY,
    };
    let driveshaft = AxleLinks {
        links: driveshaft(file),
        radius: DRIVESHAFT_RADIUS_FEET * METRES_PER_FOOT,
        texture: None,
        color: DRIVESHAFT_BROWN,
    };
    let mut lamps: Vec<TruckLamp> = file
        .lights
        .iter()
        .map(|light| lamp(light, truck, &mut textures))
        .collect();
    let corners: Vec<Vec3> = file.scrape_points.iter().copied().map(to_game).collect();
    let kinds: Vec<i32> = file.lights.iter().map(|light| light.kind).collect();
    light_the_road(&mut lamps, &kinds, &corners);

    TruckData {
        body_color: TruckData::builtin().body_color,
        lamps,
        dashboard: truck.cockpit.as_ref().and_then(dashboard).map(Arc::new),
        name: truck.file.name.clone(),
        config,
        collider_points: truck
            .file
            .scrape_points
            .iter()
            .copied()
            .map(to_game)
            .collect(),
        looks: Some(TruckLooks {
            textures: textures.made,
            body,
            left_tire,
            right_tire,
            wheel_tires,
            axle,
            axle_bars,
            shocks,
            driveshaft,
        }),
    }
}

/// The dashboard, if it has a picture to look ahead through: without one there is nothing
/// to lay the dials on.
fn dashboard(cockpit: &pod::Cockpit) -> Option<Dashboard> {
    let views: [Option<DashboardPicture>; 4] = std::array::from_fn(|way| {
        cockpit
            .backgrounds
            .get(way)
            .and_then(Option::as_ref)
            .map(dashboard_picture)
    });
    views[0].as_ref()?;
    let layout = &cockpit.layout;
    let dial = |gauge: &pod::Gauge, units_per_reading: f32| {
        Some(Dial {
            center: Vec2::from(gauge.center?),
            radius: gauge.radius?,
            zero: gauge.zero_angle?.to_radians(),
            per_unit: (gauge.degrees_per_unit? * units_per_reading).to_radians(),
        })
    };
    let most_turned = cockpit
        .steering_wheel
        .iter()
        .map(|(turn, _)| turn.unsigned_abs())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let steering_wheel = layout
        .steering_wheel
        .and_then(|[left, top, width, height]| {
            let frames: Vec<(f32, DashboardPicture)> = cockpit
                .steering_wheel
                .iter()
                .map(|(turn, picture)| (*turn as f32 / most_turned, dashboard_picture(picture)))
                .collect();
            (!frames.is_empty()).then(|| SteeringWheel {
                rect: Rect::new(left, top, left + width, top + height),
                frames,
            })
        });
    Some(Dashboard {
        screen: Vec2::new(cockpit.screen[0] as f32, cockpit.screen[1] as f32),
        window: layout
            .window
            .map(|[left, top, width, height]| Rect::new(left, top, left + width, top + height)),
        views,
        speedometer: dial(&layout.speedometer, MPH_PER_METRE_PER_SECOND),
        tachometer: dial(&layout.tachometer, 1.0),
        steering_wheel,
    })
}

/// A cockpit picture in colour, with its see-through pixels clear.
fn dashboard_picture(picture: &pod::CockpitPicture) -> DashboardPicture {
    let rgba = picture
        .picture
        .indices()
        .iter()
        .flat_map(|&index| {
            if index == pod::SEE_THROUGH_INDEX {
                return [0; 4];
            }
            let [red, green, blue] = picture.palette.color(index);
            [red, green, blue, 255]
        })
        .collect();
    DashboardPicture {
        width: picture.picture.width() as u32,
        height: picture.picture.height() as u32,
        rgba,
    }
}

/// The type of a truck file's light that its headlights have: forwards, `HEADLITE`, a
/// beam. What the types mean is **open**; this one is by how its lamps look.
const HEADLIGHT_KIND: i32 = 0;

/// Makes sure the truck has headlights that light the road, which every truck has in the
/// game. A truck whose lamps have no beam at all gets the base game's usual headlight beam
/// on its headlights, aimed as the base game's are; a truck with no headlights gets the
/// light alone, shining out in front from the front of its body (`body`, its corners), with
/// no lamps drawn on it and nothing at the back. Maximum Destruction's headlights have no
/// beam, and many community trucks have no lights. The game's own, not MTM2's.
fn light_the_road(lamps: &mut Vec<TruckLamp>, kinds: &[i32], body: &[Vec3]) {
    if lamps.iter().any(|lamp| lamp.beam.is_some()) {
        return;
    }
    let mut headlights = lamps
        .iter_mut()
        .zip(kinds)
        .filter(|(lamp, kind)| **kind == HEADLIGHT_KIND && lamp.facing.z < 0.0)
        .map(|(lamp, _)| lamp)
        .peekable();
    if headlights.peek().is_some() {
        for lamp in headlights {
            lamp.beam = Some(headlight_beam());
            lamp.facing = headlight_facing();
            lamp.color = [1.0; 3];
        }
        return;
    }
    let (min, max) = if body.len() >= 4 {
        (
            body.iter().copied().fold(Vec3::MAX, Vec3::min),
            body.iter().copied().fold(Vec3::MIN, Vec3::max),
        )
    } else {
        let half = super::display::CHASSIS_HALF_EXTENTS;
        (-half, half)
    };
    let (headlights, _) = lamps_for_body(min, max);
    // The truck's model has no lamps there to glow.
    lamps.extend(headlights.into_iter().map(|lamp| TruckLamp {
        glow_radius: 0.0,
        ..lamp
    }));
}

/// A truck file's light, as the game's lamp. Its glow is the lamp's own picture; the colour
/// of its light is its beam's texture's, or, with no beam, the picture's. See
/// `docs/formats/truck.md`.
fn lamp(light: &pod::TruckLight, truck: &pod::Truck, textures: &mut Textures) -> TruckLamp {
    // A heading turns from forwards towards the right: measured, see the format document.
    let (heading, pitch) = (light.heading, light.pitch);
    let facing = to_game([
        heading.sin() * pitch.cos(),
        pitch.sin(),
        heading.cos() * pitch.cos(),
    ])
    .normalize_or(Vec3::NEG_Z);
    let beam = (light.cone_length > 0.0).then(|| Beam {
        reach: light.cone_length * METRES_PER_FOOT,
        width: light.cone_base_radius * METRES_PER_FOOT,
        spread: (light.cone_rim_radius - light.cone_base_radius)
            .max(0.0)
            .atan2(light.cone_length),
    });
    let colour_of = if beam.is_some() {
        &light.cone_texture
    } else {
        &light.bitmap
    };
    let color = light_color(truck, colour_of)
        .or_else(|| light_color(truck, &light.bitmap))
        .unwrap_or([1.0; 3]);
    let [on, off] = light.blink_ms;
    TruckLamp {
        position: to_game(light.position),
        facing,
        // The file's heading turns to the right, and the game's turns to the left.
        spin: -light.spin,
        glow_radius: light.bitmap_radius * METRES_PER_FOOT,
        glow: textures.index(&light.bitmap, Alpha::Ignored),
        color,
        beam,
        blink: (on > 0 && off > 0).then(|| [on as f32 / 1000.0, off as f32 / 1000.0]),
    }
}

/// The colour of the light in the texture `name`: the mean of its pixels that are not
/// black, in linear light, scaled so that the brightest of red, green and blue is 1.
/// `None` for a texture the truck hasn't got, or one that is black all over.
fn light_color(truck: &pod::Truck, name: &str) -> Option<[f32; 3]> {
    let source = truck.textures.get(&name.to_ascii_uppercase())?;
    let rgba = source.texture.to_rgba(&source.palette);
    let mut sum = Vec3::ZERO;
    let mut count = 0;
    for pixel in rgba.as_chunks::<4>().0 {
        if pixel[..3] != [0, 0, 0] {
            let linear = Color::srgb_u8(pixel[0], pixel[1], pixel[2]).to_linear();
            sum += Vec3::new(linear.red, linear.green, linear.blue);
            count += 1;
        }
    }
    let mean = sum / count.max(1) as f32;
    let brightest = mean.max_element();
    (brightest > 0.0).then(|| (mean / brightest).to_array())
}

/// Four bars for each set: from each side of the body to the same side of each axle. The
/// right-hand bars meet the body at `axlebarOffset`, and the left-hand ones mirror them.
/// An MTM2.1 truck's second set is the first raised by `superiorAxlebarOffset`: at the
/// front axle, at the rear axle, and at the body.
fn axle_bars(file: &pod::TruckFile) -> Vec<AxleLink> {
    let Some([x, y, z]) = file.axle_bar_offset else {
        return Vec::new();
    };
    let lower = [0.0; 3];
    let upper = file
        .upper_axle_bar_offset
        .map(|raise| raise.map(|units| units / UPPER_BAR_UNITS_PER_FOOT));
    let mut bars = Vec::new();
    for [front_raise, rear_raise, body_raise] in [Some(lower), upper].into_iter().flatten() {
        for side in [-1.0, 1.0] {
            let body_end = to_game([side * x, y + BAR_BODY_ABOVE_FEET + body_raise, z]);
            // The front axle is at +Z in the file's axes, so the rear one is towards -Z.
            let axles = [
                (0, -BAR_AXLE_INWARD_FEET, front_raise),
                (1, BAR_AXLE_INWARD_FEET, rear_raise),
            ];
            for (axle, inward, raise) in axles {
                bars.push(AxleLink {
                    body_end,
                    axle,
                    axle_end: to_game([
                        side * BAR_AXLE_SIDE_FEET,
                        raise - BAR_AXLE_BELOW_FEET,
                        inward,
                    ]),
                });
            }
        }
    }
    bars
}

/// Two shocks at each wheel, one in front of the axle and one behind, standing from the
/// body, at the height of its origin, down to the axle. Every truck has them.
fn shocks(file: &pod::TruckFile) -> Vec<AxleLink> {
    let tires = &file.tires;
    let axles = [
        (0, tires.front_left, tires.front_right),
        (1, tires.rear_left, tires.rear_right),
    ];
    let mut shocks = Vec::new();
    for (axle, left, right) in axles {
        let [x, _, z] = middle(left, right);
        for side in [-1.0, 1.0] {
            for along in [-SHOCK_PAIR_APART_FEET, SHOCK_PAIR_APART_FEET] {
                shocks.push(AxleLink {
                    body_end: to_game([x + side * SHOCK_SIDE_FEET, 0.0, z + along]),
                    axle,
                    axle_end: to_game([side * SHOCK_SIDE_FEET, SHOCK_AXLE_ABOVE_FEET, along]),
                });
            }
        }
    }
    shocks
}

/// The driveshaft's two halves, from where the truck file puts it on the body, on the
/// truck's middle line, to the middle of each axle.
fn driveshaft(file: &pod::TruckFile) -> Vec<AxleLink> {
    let Some([_, y, z]) = file.driveshaft else {
        return Vec::new();
    };
    [0, 1]
        .map(|axle| AxleLink {
            body_end: to_game([0.0, y, z]),
            axle,
            axle_end: Vec3::ZERO,
        })
        .to_vec()
}

fn middle(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|axis| (a[axis] + b[axis]) / 2.0)
}

/// A position or a direction in a truck's own axes, in the game's. Reflecting a normal
/// along with the surface it belongs to leaves it pointing out.
fn to_game([x, up, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, up, -z) * METRES_PER_FOOT
}

/// What makes a face's mesh one of its own: its texture, colour, whether it is a cutout,
/// and how opaque it is, keyed by the bits of that number, as a float can't be, and the
/// frames of an animated texture with the bits of how long each is shown.
type PartKey = (
    Option<usize>,
    [u8; 3],
    bool,
    Option<u32>,
    Option<(Vec<usize>, u32)>,
);

fn convert_model(model: &pod::Model, textures: &mut Textures) -> TruckModel {
    let mut parts: Vec<TruckMesh> = Vec::new();
    let mut part_of: HashMap<PartKey, usize> = HashMap::new();

    for face in &model.faces {
        let texture = face
            .texture
            .as_deref()
            .and_then(|name| textures.index(name, Alpha::of(face)));
        let color = match (&face.texture, texture) {
            // Drawn through the texture as it is.
            (_, Some(_)) => [255; 3],
            (Some(_), None) => MISSING_TEXTURE_GREY,
            // A Windows COLORREF: red in the lowest byte.
            (None, None) => {
                let [red, green, blue, _] = face.color.to_le_bytes();
                [red, green, blue]
            }
        };
        let cutout = texture.is_some() && face.is_cutout();
        let opacity = face.blend_alpha();
        let texture_cycle = face
            .texture_cycle
            .and_then(|index| model.texture_cycles.get(index))
            .filter(|_| texture.is_some())
            .and_then(|cycle| texture_cycle(cycle, face, textures));
        let key = (
            texture,
            color,
            cutout,
            opacity.map(f32::to_bits),
            texture_cycle
                .as_ref()
                .map(|cycle| (cycle.frames.clone(), cycle.seconds_per_frame.to_bits())),
        );
        let part = *part_of.entry(key).or_insert_with(|| {
            parts.push(TruckMesh {
                texture,
                texture_cycle,
                color,
                cutout,
                opacity,
                ..default()
            });
            parts.len() - 1
        });
        let mesh = &mut parts[part];

        // Reversed, because flipping Z turns the face inside out. A two-sided face is
        // also drawn as written, which is its back, since the material culls back faces.
        let reversed: Vec<&pod::Corner> = face.corners.iter().rev().collect();
        let as_written: Vec<&pod::Corner> = face.corners.iter().collect();
        let sides = if face.is_two_sided() { 2 } else { 1 };
        for corners in [reversed, as_written].into_iter().take(sides) {
            add_face(mesh, model, face, &corners);
        }
    }
    TruckModel { parts }
}

/// An animated texture's frames, as textures of the truck. `None`, for a texture that stays
/// on its first frame, where a frame is missing, or the cycle has one frame or no time.
fn texture_cycle(
    cycle: &pod::TextureCycle,
    face: &pod::Face,
    textures: &mut Textures,
) -> Option<TruckTextureCycle> {
    if cycle.frames.len() < 2 || cycle.seconds_per_frame.is_nan() || cycle.seconds_per_frame <= 0.0
    {
        return None;
    }
    let frames = cycle
        .frames
        .iter()
        .map(|name| textures.index(name, Alpha::of(face)))
        .collect::<Option<Vec<_>>>()?;
    Some(TruckTextureCycle {
        frames,
        seconds_per_frame: cycle.seconds_per_frame,
    })
}

/// One face, seen from the side its corners wind anticlockwise round.
fn add_face(mesh: &mut TruckMesh, model: &pod::Model, face: &pod::Face, corners: &[&pod::Corner]) {
    let positions: Vec<Vec3> = corners
        .iter()
        .map(|corner| to_game(model.vertices[corner.vertex]))
        .collect();
    // Summed over every edge (Newell's method) rather than taken from the first three
    // corners, which in some faces lie in a line.
    let face_normal = (0..positions.len())
        .map(|index| positions[index].cross(positions[(index + 1) % positions.len()]))
        .sum::<Vec3>()
        .normalize_or_zero();

    let first = mesh.positions.len() as u32;
    for (corner, position) in corners.iter().zip(&positions) {
        // The back of a two-sided panel shares its vertices, and so their normals,
        // with the front. Those, and the zero normals some tools leave, give way to
        // the face's own.
        let normal = model
            .vertex_normals
            .get(corner.vertex)
            .filter(|_| face.is_smooth())
            .map(|&normal| to_game(normal).normalize_or_zero())
            .filter(|normal| normal.dot(face_normal) > 0.0)
            .unwrap_or(face_normal);
        mesh.positions.push(position.to_array());
        mesh.normals.push(normal.to_array());
        // A few files hold rubbish for texture coordinates; keep it on the texture.
        mesh.uvs.push(corner.uv.map(|along| {
            if along.is_finite() {
                along.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }));
    }
    // A fan from the first corner. The game's own faces are flat and convex.
    for second in 1..corners.len() as u32 - 1 {
        mesh.indices
            .extend([first, first + second, first + second + 1]);
    }
}

/// What a face does with its texture's alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Alpha {
    /// Nothing: the face is opaque.
    Ignored,
    /// Holes where it is low: pure black, for an 8-bit texture.
    Cutout,
    /// See-through by as much, as glass is. An 8-bit texture has none.
    Blended,
}

impl Alpha {
    fn of(face: &pod::Face) -> Self {
        if face.is_cutout() {
            Self::Cutout
        } else if face.blend_alpha().is_some() {
            Self::Blended
        } else {
            Self::Ignored
        }
    }
}

/// The textures made so far, and how to make more.
struct Textures<'a> {
    truck: &'a pod::Truck,
    made: Vec<TruckTexture>,
    known: HashMap<(String, Alpha), usize>,
}

impl Textures<'_> {
    /// The number of a texture, or `None` if the archive doesn't have it or its palette.
    /// A texture used in more than one way (see `Alpha`) becomes one texture for each.
    fn index(&mut self, name: &str, alpha: Alpha) -> Option<usize> {
        let key = (name.to_ascii_uppercase(), alpha);
        if let Some(&index) = self.known.get(&key) {
            return Some(index);
        }
        let texture = self.hd_texture(&key.0, alpha).or_else(|| {
            let source = self.truck.textures.get(&key.0)?;
            let mut rgba = source.texture.to_rgba(&source.palette);
            if alpha == Alpha::Cutout {
                // MTM2 has no alpha. On a cutout face, pure black is a hole. It stays
                // black so that smooth filtering doesn't draw a bright fringe round it.
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    if pixel[..3] == [0, 0, 0] {
                        pixel[3] = 0;
                    }
                }
            }
            Some(TruckTexture {
                size: source.texture.size(),
                rgba,
                normal_map: None,
            })
        })?;
        let normal_map = self.truck.normal_maps.get(&key.0).and_then(|source| {
            Some(NormalMap {
                size: source.size(),
                rgba: hd_texture::decode(source)?,
            })
        });
        self.made.push(TruckTexture {
            normal_map,
            ..texture
        });
        self.known.insert(key, self.made.len() - 1);
        Some(self.made.len() - 1)
    }

    /// The texture from its PNG or TGA, where the archive has one, as the patched game
    /// uses it. That brings its own alpha, which a cutout or blended face keeps and any
    /// other face ignores. **Reference:** JSTruckViewer, `docs/BIN_HD_FORMAT.md`.
    fn hd_texture(&self, name: &str, alpha: Alpha) -> Option<TruckTexture> {
        let source = self.truck.hd_textures.get(name)?;
        let mut rgba = hd_texture::decode(source)?;
        if alpha == Alpha::Ignored {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
        }
        Some(TruckTexture {
            size: source.size(),
            rgba,
            normal_map: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const FOOT: f32 = METRES_PER_FOOT;

    fn truck_file() -> pod::TruckFile {
        pod::TruckFile {
            name: "Test Foot".into(),
            extended: false,
            body_model: "foot.bin".into(),
            tire_model_base: "FOOT_".into(),
            axle_model: "axle.bin".into(),
            bar_texture: None,
            shock_texture: None,
            driveshaft: None,
            axle_bar_offset: None,
            upper_axle_bar_offset: None,
            tires: pod::Tires {
                front_left: [-4.5, -3.5, 6.5],
                front_right: [4.5, -3.5, 6.5],
                rear_left: [-4.5, -3.5, -6.0],
                rear_right: [4.5, -3.5, -6.0],
            },
            scrape_points: vec![[-3.5, 0.0, 9.5], [3.5, 5.0, -9.0]],
            lights: Vec::new(),
            instrument_cluster: None,
        }
    }

    fn truck(body: Option<pod::Model>, tire: Option<pod::Model>) -> pod::Truck {
        pod::Truck {
            file: truck_file(),
            body,
            left_tire: tire,
            right_tire: None,
            wheel_tires: [None, None, None, None],
            axle: None,
            textures: BTreeMap::new(),
            hd_textures: BTreeMap::new(),
            normal_maps: BTreeMap::new(),
            cockpit: None,
        }
    }

    /// One triangle on the truck's right side, looking right (+X), wound as the files
    /// wind them: by the right-hand rule on the numbers as written.
    fn panel(kind: i32, vertex_normals: Vec<[f32; 3]>) -> pod::Model {
        let corner = |vertex| pod::Corner {
            vertex,
            uv: [0.25, 7.0],
        };
        pod::Model {
            vertices: vec![[3.0, 0.0, 0.0], [3.0, 1.0, 0.0], [3.0, 0.0, 1.0]],
            vertex_normals,
            faces: vec![pod::Face {
                kind,
                texture: None,
                texture_cycle: None,
                color: 0x0040_80ff,
                material: None,
                corners: vec![corner(0), corner(1), corner(2)],
            }],
            texture_cycles: Vec::new(),
            incomplete: None,
        }
    }

    #[test]
    fn a_truck_faces_minus_z_with_its_right_at_plus_x() {
        let data = truck_from_pod(&truck(None, None));
        assert_eq!(data.name, "Test Foot");
        let [front_left, front_right, rear_left, _] = data.config.wheel_rest;
        assert!(front_left.x < 0.0 && front_right.x > 0.0);
        assert!(front_left.z < 0.0 && rear_left.z > 0.0);
        assert!((front_right.x - 4.5 * FOOT).abs() < 1e-5);
        assert!((front_right.z + 6.5 * FOOT).abs() < 1e-5);
        assert_eq!(
            data.collider_points,
            [
                Vec3::new(-3.5, 0.0, -9.5) * FOOT,
                Vec3::new(3.5, 5.0, 9.0) * FOOT
            ]
        );
    }

    #[test]
    fn the_truck_stands_where_the_file_says_it_does() {
        let data = truck_from_pod(&truck(None, None));
        let config = &data.config;
        // A hub rests where the file has the tire, and the body is dropped onto it.
        let hub = config.wheel_rest[0].y;
        assert!((hub + 3.5 * FOOT - BODY_DROP).abs() < 1e-5, "{hub}");

        // As far above the axles as the built-in truck's, and midway between them.
        let builtin = TruckConfig::default();
        let builtin_hub = builtin.wheel_rest[0].y;
        assert!(
            ((config.center_of_mass.y - hub) - (builtin.center_of_mass.y - builtin_hub)).abs()
                < 1e-5
        );
        assert!((config.center_of_mass.z + 0.25 * FOOT).abs() < 1e-5);
        assert_eq!(config.center_of_mass.x, 0.0);
        // Nothing in the file says how it drives.
        assert_eq!(config.mass, builtin.mass);
        assert_eq!(config.wheel_radius, builtin.wheel_radius);
    }

    #[test]
    fn the_wheel_is_as_big_as_the_tire_model() {
        let mut tire = panel(24, Vec::new());
        tire.vertices = vec![[-2.0, -3.0, -3.0], [2.0, 3.0, 3.0], [0.0, 0.0, 3.0]];
        let data = truck_from_pod(&truck(None, Some(tire)));
        assert!((data.config.wheel_radius - 3.0 * FOOT).abs() < 1e-6);
        assert!((data.config.wheel_width - 4.0 * FOOT).abs() < 1e-6);
    }

    #[test]
    fn faces_keep_facing_outwards_when_z_is_flipped() {
        let data = truck_from_pod(&truck(Some(panel(24, Vec::new())), None));
        let body = data.looks.unwrap().body.unwrap();
        let [mesh] = &body.parts[..] else {
            panic!("one mesh expected");
        };
        assert_eq!((mesh.texture, mesh.color), (None, [0xff, 0x80, 0x40]));
        assert_eq!(mesh.normals, [[1.0, 0.0, 0.0]; 3]);
        // Counter-clockwise seen from the right of the truck, where the face looks.
        let [a, b, c] =
            [0, 1, 2].map(|index| Vec3::from(mesh.positions[mesh.indices[index] as usize]));
        assert!((b - a).cross(c - a).x > 0.0);
        assert!(mesh.positions.contains(&[3.0 * FOOT, 0.0, -FOOT]));
        assert_eq!(mesh.uvs[0], [0.25, 1.0]);
    }

    #[test]
    fn a_two_sided_face_is_drawn_from_behind_as_well() {
        let mut model = panel(pod::face_type::MATERIAL, Vec::new());
        model.faces[0].material = Some(pod::Material {
            flags: pod::material_flag::TWO_SIDED,
            base_alpha: 1.0,
        });
        let data = truck_from_pod(&truck(Some(model), None));
        let mesh = &data.looks.unwrap().body.unwrap().parts[0];
        assert_eq!(mesh.indices.len(), 6);
        assert_eq!(mesh.normals[..3], [[1.0, 0.0, 0.0]; 3]);
        assert_eq!(mesh.normals[3..], [[-1.0, 0.0, 0.0]; 3]);
    }

    /// The GMC's glass: flags 0x167 (blend), base alpha 0.35.
    #[test]
    fn a_blended_face_is_a_mesh_of_its_own_that_says_how_opaque_it_is() {
        let mut model = panel(pod::face_type::MATERIAL, Vec::new());
        let mut glass = model.faces[0].clone();
        glass.material = Some(pod::Material {
            flags: 0x167,
            base_alpha: 0.35,
        });
        model.faces.push(glass);
        let data = truck_from_pod(&truck(Some(model), None));
        let parts = data.looks.unwrap().body.unwrap().parts;
        let opacities: Vec<_> = parts.iter().map(|part| part.opacity).collect();
        assert_eq!(opacities, [None, Some(0.35)]);
    }

    /// The GMC's offsets: a set of four bars, and a second set raised by 200, 200 and
    /// 400 units of 1/256 ft at the front axle, the rear axle and the body.
    #[test]
    fn axle_bars_run_from_the_body_to_each_axle() {
        let mut file = truck_file();
        assert!(axle_bars(&file).is_empty());

        file.axle_bar_offset = Some([1.37125, -2.7, -0.113281]);
        let lower = axle_bars(&file);
        assert_eq!(lower.len(), 4);
        // Z is flipped: the front axle's end is behind its middle, at +Z here.
        let right_front = lower
            .iter()
            .find(|bar| bar.axle == 0 && bar.body_end.x > 0.0)
            .unwrap();
        let expected_body = Vec3::new(1.37125, -2.7 + 45.0 / 256.0, 0.113281) * FOOT;
        assert!(right_front.body_end.distance(expected_body) < 1e-5);
        let expected_axle = Vec3::new(535.0, -80.0, 83.0) / 256.0 * FOOT;
        assert!(right_front.axle_end.distance(expected_axle) < 1e-5);
        // Left mirrors right.
        let left_front = lower
            .iter()
            .find(|bar| bar.axle == 0 && bar.body_end.x < 0.0)
            .unwrap();
        assert_eq!(left_front.body_end.x, -right_front.body_end.x);
        assert_eq!(left_front.axle_end.x, -right_front.axle_end.x);

        file.upper_axle_bar_offset = Some([200.0, 200.0, 400.0]);
        let both = axle_bars(&file);
        assert_eq!(both.len(), 8);
        let upper = &both[4..];
        let raised = |bar: &AxleLink, other: &AxleLink| {
            (
                bar.body_end.y - other.body_end.y,
                bar.axle_end.y - other.axle_end.y,
            )
        };
        let (body, axle) = raised(&upper[0], &lower[0]);
        assert!((body - 400.0 / 256.0 * FOOT).abs() < 1e-5);
        assert!((axle - 200.0 / 256.0 * FOOT).abs() < 1e-5);
    }

    /// The test truck's tires are 4.5 ft either side, 3.5 ft down and 6.5 ft ahead at the
    /// front.
    #[test]
    fn shocks_stand_from_the_body_down_to_each_axle() {
        let shocks = shocks(&truck_file());
        assert_eq!(shocks.len(), 8);
        let front_right: Vec<_> = shocks
            .iter()
            .filter(|shock| shock.axle == 0 && shock.axle_end.x > 0.0)
            .collect();
        assert_eq!(front_right.len(), 2);
        for shock in front_right {
            // Level with the body's origin, over the axle's middle, 70/256 ft either way.
            assert_eq!(shock.body_end.y, 0.0);
            assert!((shock.body_end.x - 542.0 / 256.0 * FOOT).abs() < 1e-5);
            assert!(((shock.body_end.z + 6.5 * FOOT).abs() - 70.0 / 256.0 * FOOT).abs() < 1e-5);
            assert!((shock.axle_end.y - 85.0 / 256.0 * FOOT).abs() < 1e-5);
        }
    }

    #[test]
    fn the_driveshaft_runs_from_the_body_to_the_middle_of_each_axle() {
        let mut file = truck_file();
        assert!(driveshaft(&file).is_empty());
        file.driveshaft = Some([0.0, -2.31625, -0.113281]);
        let halves = driveshaft(&file);
        assert_eq!(halves.len(), 2);
        assert_eq!((halves[0].axle, halves[1].axle), (0, 1));
        assert!(halves.iter().all(|half| half.axle_end == Vec3::ZERO));
        let expected = Vec3::new(0.0, -2.31625, 0.113281) * FOOT;
        assert!(halves[0].body_end.distance(expected) < 1e-5);
    }

    /// An MTM2.1 truck's rear tires may be models of their own, which the wheel size
    /// comes from as much as the side's.
    #[test]
    fn a_wheel_with_a_tire_of_its_own_gets_it() {
        let mut rear = panel(24, Vec::new());
        rear.vertices = vec![[-2.0, -3.0, -3.0], [2.0, 3.0, 3.0], [0.0, 0.0, 3.0]];
        let mut pod_truck = truck(None, None);
        pod_truck.wheel_tires[2] = Some(rear);
        let data = truck_from_pod(&pod_truck);
        assert!((data.config.wheel_radius - 3.0 * FOOT).abs() < 1e-6);
        let looks = data.looks.unwrap();
        assert!(looks.wheel_tires[2].is_some());
        assert!(looks.wheel_tires[0].is_none() && looks.left_tire.is_none());
    }

    /// As the engine covers of Captain USA and three other trucks: two frames at rate 2729.
    #[test]
    fn an_animated_texture_is_a_mesh_that_steps_through_its_frames() {
        let mut body = panel(24, Vec::new());
        body.faces[0].texture = Some("AENGANI1.RAW".into());
        body.faces[0].texture_cycle = Some(0);
        body.texture_cycles = vec![pod::TextureCycle {
            frames: vec!["AENGANI1.RAW".into(), "AENGANI2.RAW".into()],
            seconds_per_frame: 2729.0 / 65536.0,
        }];
        let mut still = body.faces[0].clone();
        still.texture_cycle = None;
        body.faces.push(still);

        let mut truck = truck(Some(body), None);
        for (name, index) in [("AENGANI1.RAW", 1), ("AENGANI2.RAW", 2)] {
            truck.textures.insert(
                name.into(),
                pod::TruckTexture {
                    texture: pod::Texture::parse(&[index; 4]).unwrap(),
                    palette: pod::Palette::parse(&[index; 768]).unwrap(),
                },
            );
        }
        let looks = truck_from_pod(&truck).looks.unwrap();
        let parts = &looks.body.unwrap().parts;
        // The animated face and the still one with the same texture are meshes apart.
        assert_eq!(parts.len(), 2);
        let cycle = parts[0].texture_cycle.as_ref().unwrap();
        assert_eq!(parts[0].texture, Some(cycle.frames[0]));
        assert_eq!(cycle.frames.len(), 2);
        assert_ne!(cycle.frames[0], cycle.frames[1]);
        assert_eq!(cycle.seconds_per_frame, 2729.0 / 65536.0);
        assert_eq!(parts[1].texture_cycle, None);

        // A missing frame leaves it still.
        truck.textures.remove("AENGANI2.RAW");
        let looks = truck_from_pod(&truck).looks.unwrap();
        assert_eq!(looks.body.unwrap().parts[0].texture_cycle, None);
    }

    #[test]
    fn smooth_faces_use_their_vertex_normals_where_those_agree() {
        let slanted = [0.6, 0.8, 0.0];
        let normals = vec![slanted, [-1.0, 0.0, 0.0], [0.0; 3]];
        let smooth = truck_from_pod(&truck(Some(panel(41, normals.clone())), None));
        let mesh = &smooth.looks.unwrap().body.unwrap().parts[0];
        // Corners come out in reverse. The one facing the wrong way, which belongs to the
        // other side of a two-sided panel, and the zero one get the face's normal.
        assert_eq!(mesh.normals[2], slanted);
        assert_eq!(mesh.normals[1], [1.0, 0.0, 0.0]);
        assert_eq!(mesh.normals[0], [1.0, 0.0, 0.0]);

        let flat = truck_from_pod(&truck(Some(panel(24, normals)), None));
        let mesh = &flat.looks.unwrap().body.unwrap().parts[0];
        assert_eq!(mesh.normals, [[1.0, 0.0, 0.0]; 3]);
    }

    #[test]
    fn every_truck_is_given_headlights_that_light_the_road() {
        let body = [Vec3::new(-1.0, 0.0, -2.0), Vec3::new(1.0, 1.0, 2.0)].repeat(2);
        // No lamps at all: the light alone, from the front, and nothing at the back.
        let mut none = Vec::new();
        light_the_road(&mut none, &[], &body);
        assert_eq!(none.len(), 2);
        assert!(
            none.iter().all(|lamp| lamp.beam.is_some()
                && lamp.position.z == -2.0
                && lamp.glow_radius == 0.0)
        );

        // Headlights with no beam are given one, aimed a little down.
        let glow_only = TruckLamp {
            beam: None,
            facing: Vec3::new(0.0, 0.4, -1.0).normalize(),
            ..none[0].clone()
        };
        let tail = TruckLamp {
            beam: None,
            facing: Vec3::Z,
            ..none[0].clone()
        };
        let mut lamps = vec![glow_only.clone(), tail.clone()];
        light_the_road(&mut lamps, &[HEADLIGHT_KIND, 1], &body);
        assert_eq!(lamps.len(), 2);
        assert!(lamps[0].beam.is_some() && lamps[0].facing.y < 0.0);
        assert!(lamps[1].beam.is_none());

        // A truck whose lamps have beams keeps them as they are.
        let mut lit = none.clone();
        light_the_road(&mut lit, &[0, 0], &body);
        assert_eq!(lit, none);
    }

    /// A picture of palette indices, and a palette where index `i` is grey `i`.
    fn cockpit_picture(width: usize, height: usize, indices: &[u8]) -> pod::CockpitPicture {
        let greys: Vec<u8> = (0..=255u8).flat_map(|grey| [grey; 3]).collect();
        pod::CockpitPicture {
            picture: pod::Picture::parse(indices, width, height).unwrap(),
            palette: pod::Palette::parse(&greys).unwrap(),
        }
    }

    #[test]
    fn a_cockpit_becomes_a_dashboard_in_the_games_units() {
        let background = cockpit_picture(2, 1, &[0, 7]);
        let wheel = |index| cockpit_picture(1, 1, &[index]);
        let mut layout = pod::CockpitLayout {
            window: Some([0.0, 10.0, 2.0, 20.0]),
            steering_wheel: Some([1.0, 2.0, 3.0, 4.0]),
            ..Default::default()
        };
        layout.speedometer = pod::Gauge {
            center: Some([1.0, 2.0]),
            radius: Some(3.0),
            zero_angle: Some(180.0),
            degrees_per_unit: Some(2.0),
            ..Default::default()
        };
        // No radius: no tachometer.
        layout.tachometer = pod::Gauge {
            center: Some([1.0, 2.0]),
            ..layout.speedometer.clone()
        };
        layout.tachometer.radius = None;
        let cockpit = pod::Cockpit {
            file: "POWERBIG.480".into(),
            screen: [640, 480],
            layout,
            backgrounds: vec![Some(background), None, Some(cockpit_picture(1, 1, &[3]))],
            steering_wheel: vec![(-35, wheel(1)), (0, wheel(2)), (35, wheel(3))],
        };
        let converted = dashboard(&cockpit).unwrap();
        assert_eq!(converted.screen, Vec2::new(640.0, 480.0));
        assert_eq!(converted.window, Some(Rect::new(0.0, 10.0, 2.0, 30.0)));
        // Index 0 is see-through; the rest is coloured by the palette.
        let ahead = converted.views[0].as_ref().unwrap();
        assert_eq!((ahead.width, ahead.height), (2, 1));
        assert_eq!(ahead.rgba, [0, 0, 0, 0, 7, 7, 7, 255]);
        assert!(converted.views[1].is_none() && converted.views[3].is_none());
        assert!(converted.views[2].is_some());

        let speedometer = converted.speedometer.unwrap();
        assert!((speedometer.zero - std::f32::consts::PI).abs() < 1e-6);
        // 2 degrees per mph is 4.47 degrees per metre a second.
        let per_metre_per_second = speedometer.per_unit.to_degrees();
        assert!((per_metre_per_second - 2.0 * 2.236_936).abs() < 1e-3);
        assert!(converted.tachometer.is_none());

        let steering_wheel = converted.steering_wheel.unwrap();
        assert_eq!(steering_wheel.rect, Rect::new(1.0, 2.0, 4.0, 6.0));
        let turns: Vec<f32> = steering_wheel
            .frames
            .iter()
            .map(|(turn, _)| *turn)
            .collect();
        assert_eq!(turns, [-1.0, 0.0, 1.0]);

        // Without a picture to look ahead through, there is no dashboard.
        let mut blind = cockpit.clone();
        blind.backgrounds[0] = None;
        assert!(dashboard(&blind).is_none());
    }
}
