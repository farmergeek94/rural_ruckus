//! The truck file (.TRK): what a truck is made of and where its parts go. See
//! `docs/formats/truck.md`.
//!
//! It is text, laid out like a track file: each value sits on the line after a label that
//! names it. It describes the truck's shape and its lights, and nothing of how it drives.
//! Only what the game uses so far is read, so the sounds are skipped.

use super::PodError;
use super::situation::Lines;

/// What an MTM2 truck file starts with, and the label of the truck's name.
const NAME_LABEL: &str = "MTM2 truckName";
/// The same, in a truck file for MTM2 Community Patch 3's extended trucks. **Reference:**
/// JSTruckViewer, `docs/TRK_2_1_FORMAT.md`: the rest of the file is laid out as before.
const EXTENDED_NAME_LABEL: &str = "MTM2.1 truckName";
const SCRAPE_POINT_LABEL: &str = "Scrape point";

#[derive(Clone, Debug, PartialEq)]
pub struct TruckFile {
    pub name: String,
    /// An MTM2.1 truck file, which may give each wheel a tire model of its own. See
    /// `wheel_tire_model_name`.
    pub extended: bool,
    /// File name of the body's model (.BIN).
    pub body_model: String,
    /// What the file names of the tire models begin with. See `tire_model_names`.
    pub tire_model_base: String,
    /// File name of the model (.BIN) used for both axles.
    pub axle_model: String,
    /// The texture of the bars that hold the axles, as written (`axlebar2.raw`).
    pub bar_texture: Option<String>,
    /// The texture of the shock absorbers, as written (`shockss.raw`).
    pub shock_texture: Option<String>,
    /// Where the driveshaft meets the body, in feet, in the body's axes; it runs from
    /// there to the middle of each axle. `None` for a file that doesn't say, or that
    /// writes all zeros, as trucks without one do.
    pub driveshaft: Option<[f32; 3]>,
    /// Where each bar that holds an axle meets the body, in feet, in the body's axes: the
    /// right-hand bars, which the left-hand ones mirror. `None` for a file that doesn't
    /// say, or that puts them far out of sight, as trucks without axle bars do.
    pub axle_bar_offset: Option<[f32; 3]>,
    /// How much higher an MTM2.1 truck's second set of axle bars is than its first, in
    /// 1/256 ft: at the front axle, at the rear axle, and where they meet the body. `None`
    /// for a truck with one set.
    pub upper_axle_bar_offset: Option<[f32; 3]>,
    pub tires: Tires,
    /// The corners of the body, where it touches what it runs into. In feet, in the
    /// body's axes.
    pub scrape_points: Vec<[f32; 3]>,
    /// The lamps on the body, in the order the file numbers them. A light whose lines can
    /// not be read is left out.
    pub lights: Vec<TruckLight>,
    /// What the dashboard's layout file is called, without its extension (`powerbig`):
    /// see `docs/formats/cockpit.md`. `None` for a file that doesn't say.
    pub instrument_cluster: Option<String>,
}

/// One of a truck's lights: a picture drawn at the lamp, and, for some, a beam. Every
/// field is **measured**: its label names it and gives its unit. What `kind` means is
/// **open**. See `docs/formats/truck.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct TruckLight {
    /// `Light N type`. 0, 1, 3, 4 and 5 occur.
    pub kind: i32,
    /// Where the lamp is, in feet, in the body's axes.
    pub position: [f32; 3],
    /// How big its picture is drawn, in feet from its middle to its edge.
    pub bitmap_radius: f32,
    /// Which way it shines, in radians: round from straight ahead, and up from level.
    pub heading: f32,
    pub pitch: f32,
    /// How fast its heading turns, in radians a second: a beacon that goes round.
    pub spin: f32,
    /// The beam: how long it is and how wide at the lamp and at its far end, in feet, and
    /// its texture. A length of 0 is no beam.
    pub cone_length: f32,
    pub cone_base_radius: f32,
    pub cone_rim_radius: f32,
    pub cone_texture: String,
    /// The picture drawn at the lamp.
    pub bitmap: String,
    /// For a light that blinks, how long it is on and then off, in milliseconds. Both 0
    /// for a light that does not.
    pub blink_ms: [u32; 2],
}

/// Where the middle of each tire is with the truck standing at rest. In feet, in the
/// body's axes: X to the truck's right, Y up, Z forwards, from the body model's origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Tires {
    pub front_left: [f32; 3],
    pub front_right: [f32; 3],
    pub rear_left: [f32; 3],
    pub rear_right: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// An axle bar offset this far from the body, in feet, in any direction, means "no axle
/// bars": longer than any truck, so the bars would be out of sight. **Reference:**
/// JSTruckViewer, `src/worker/truck-worker.js`, which found offsets of 2.2 to 3.3 ft in
/// all 20 of the base game's trucks, and 999 ft in a car.
const NO_AXLE_BARS_FEET: f32 = 50.0;

/// How finely a tire model is made, finest first. Each tire comes in all three.
const TIRE_DETAILS: [&str; 3] = ["16", "10", "08"];

/// Which wheel, for a tire model made for one wheel alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wheel {
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

/// Whether a truck file starts with the MTM2.1 header rather than the MTM2 one, or an
/// error if it starts with neither.
fn header(lines: &Lines) -> Result<bool, PodError> {
    match lines.label(0) {
        NAME_LABEL => Ok(false),
        EXTENDED_NAME_LABEL => Ok(true),
        _ => Err(PodError::Unsupported(format!(
            "a truck file that starts with \"{}\" instead of \"{NAME_LABEL}\"",
            lines.text(0)
        ))),
    }
}

impl TruckFile {
    /// The truck's name and nothing else: what a list of trucks needs, from a file whose
    /// other lines may be beyond reading.
    pub fn parse_name(text: &str) -> Result<String, PodError> {
        let lines = Lines::new(text);
        header(&lines)?;
        Ok(lines.text(1).to_string())
    }

    pub fn parse(text: &str) -> Result<Self, PodError> {
        let lines = Lines::new(text);
        let end = lines.len();
        let extended = header(&lines)?;
        let value = |label: &'static str| lines.after(label, 0, end).map(|line| lines.text(line));
        let tire = |tire: &str| -> Result<[f32; 3], PodError> {
            let mut position = [0.0; 3];
            for (axis, letter) in ["x", "y", "z"].into_iter().enumerate() {
                let label = format!("{tire}.static_bpos.{letter}");
                let line = (0..end).find(|&index| lines.label(index) == label).ok_or(
                    PodError::MissingField {
                        field: label.clone(),
                    },
                )?;
                [position[axis]] = lines.floats(&label, line + 1)?;
            }
            Ok(position)
        };

        let optional_floats = |label: &'static str| -> Result<Option<[f32; 3]>, PodError> {
            match lines.after(label, 0, end) {
                Ok(line) => lines.floats(label, line).map(Some),
                Err(_) => Ok(None),
            }
        };
        let axle_bar_offset = optional_floats("axlebarOffset")?
            .filter(|offset| offset.iter().all(|part| part.abs() < NO_AXLE_BARS_FEET));
        let driveshaft = optional_floats("driveshaftPos")?.filter(|position| position != &[0.0; 3]);
        // The patched game draws the second set only for a file that asks for MTM2.1.
        let upper_axle_bar_offset = optional_floats("superiorAxlebarOffset")?
            .filter(|_| extended && axle_bar_offset.is_some());

        let mut scrape_points = Vec::new();
        for index in 0..end {
            if lines.label(index).starts_with(SCRAPE_POINT_LABEL) {
                scrape_points.push(lines.floats(SCRAPE_POINT_LABEL, index + 1)?);
            }
        }

        let lights = lights(&lines);

        Ok(Self {
            name: lines.text(1).to_string(),
            extended,
            body_model: model_file_name(value("truckModelBaseName")?),
            tire_model_base: value("tireModelBaseName")?.to_string(),
            axle_model: model_file_name(value("axleModelName")?),
            bar_texture: value("barTextureName").ok().map(str::to_string),
            shock_texture: value("shockTextureName").ok().map(str::to_string),
            driveshaft,
            axle_bar_offset,
            upper_axle_bar_offset,
            tires: Tires {
                front_left: tire("faxle.ltire")?,
                front_right: tire("faxle.rtire")?,
                rear_left: tire("raxle.ltire")?,
                rear_right: tire("raxle.rtire")?,
            },
            scrape_points,
            lights,
            instrument_cluster: value("Instrument Cluster")
                .ok()
                .filter(|name| !name.is_empty())
                .map(str::to_string),
        })
    }

    /// The file name of the tire model made for one wheel alone, which an MTM2.1 truck
    /// may have, and which the game then uses in place of the one for its side. Only the
    /// finest detail has these. `None` for an MTM2 truck: the patched game ignores such
    /// models unless the file asks for them. **Reference:** JSTruckViewer,
    /// `docs/TRK_2_1_FORMAT.md`.
    pub fn wheel_tire_model_name(&self, wheel: Wheel) -> Option<String> {
        let place = match wheel {
            Wheel::FrontLeft => "FL",
            Wheel::FrontRight => "FR",
            Wheel::RearLeft => "RL",
            Wheel::RearRight => "RR",
        };
        let finest = TIRE_DETAILS[0];
        self.extended
            .then(|| format!("{}{finest}{place}.BIN", self.tire_model_base))
    }

    /// File names the model of a tire on this side may have, finest first.
    pub fn tire_model_names(&self, side: Side) -> Vec<String> {
        let side = match side {
            Side::Left => "L",
            Side::Right => "R",
        };
        TIRE_DETAILS
            .iter()
            .map(|detail| format!("{}{detail}{side}.BIN", self.tire_model_base))
            .collect()
    }
}

/// The lights that `Number of Lights` counts. Each is a run of labelled lines, `Light N
/// type` and so on, and is looked for by its number, so that a light with a line missing
/// or bad is left out and the rest are still read.
fn lights(lines: &Lines) -> Vec<TruckLight> {
    let Ok(count_line) = lines.after("Number of Lights", 0, lines.len()) else {
        return Vec::new();
    };
    let count: usize = lines.text(count_line).parse().unwrap_or(0);
    (0..count)
        .filter_map(|number| light(lines, number))
        .collect()
}

fn light(lines: &Lines, number: usize) -> Option<TruckLight> {
    let prefix = format!("Light {number} ");
    // The value of the line labelled `Light N <what>...`, split at its commas.
    let value = |what: &str| -> Option<Vec<&str>> {
        let label = format!("{prefix}{what}");
        let index = (0..lines.len()).find(|&index| lines.label(index).starts_with(&label))?;
        Some(lines.text(index + 1).split(',').map(str::trim).collect())
    };
    let number = |text: &str| text.parse::<f32>().ok();
    let kind = value("type")?.first()?.parse().ok()?;
    let [x, y, z, bitmap_radius] = value("body axis pos")?[..] else {
        return None;
    };
    let [heading, pitch, spin] = value("heading")?[..] else {
        return None;
    };
    let [length, base, rim, cone_texture] = value("cone")?[..] else {
        return None;
    };
    let bitmap = value("source")?.join(",");
    let blink = value("ms on")?;
    let blink_ms = [blink.first()?.parse().ok()?, blink.get(1)?.parse().ok()?];
    Some(TruckLight {
        kind,
        position: [number(x)?, number(y)?, number(z)?],
        bitmap_radius: number(bitmap_radius)?,
        heading: number(heading)?,
        pitch: number(pitch)?,
        spin: number(spin)?,
        cone_length: number(length)?,
        cone_base_radius: number(base)?,
        cone_rim_radius: number(rim)?,
        cone_texture: cone_texture.to_string(),
        bitmap,
        blink_ms,
    })
}

/// Some truck files name a model in full and some leave the extension off.
fn model_file_name(written: &str) -> String {
    if written.contains('.') {
        written.to_string()
    } else {
        format!("{written}.bin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cut down from a real truck, keeping its layout: CRLF line endings, a tire's three
    /// coordinates spread among the other tires', and the lights at the end.
    const TRUCK: &str = "MTM2 truckName\r\nTest Foot\r\ntruckModelBaseName\r\nfoot.bin\r\n\
tireModelBaseName\r\nFOOT_\r\naxleModelName\r\naxle3\r\nshockTextureName\r\nSILVER.RAW\r\n\
axlebarOffset\r\n1.25,-2.5,0.0\r\n\
faxle.rtire.static_bpos.x\r\n4.5\r\nfaxle.ltire.static_bpos.x\r\n-4.5\r\n\
raxle.rtire.static_bpos.x\r\n4.25\r\nraxle.ltire.static_bpos.x\r\n-4.25\r\n\
faxle.rtire.static_bpos.y\r\n-3.5\r\nfaxle.ltire.static_bpos.y\r\n-3.5\r\n\
raxle.rtire.static_bpos.y\r\n-3.75\r\nraxle.ltire.static_bpos.y\r\n-3.75\r\n\
faxle.rtire.static_bpos.z\r\n6.5\r\nfaxle.ltire.static_bpos.z\r\n6.5\r\n\
raxle.rtire.static_bpos.z\r\n-6.25\r\nraxle.ltire.static_bpos.z\r\n-6.25\r\n\
Scrape point 1 body axis x,y,z\r\n-3.75,0.0,9.5\r\nScrape point 2 body axis x,y,z\r\n3.75,2.5,-9.5\r\n\
Instrument Cluster\r\npowerbig\r\nNumber of Lights\r\n2\r\n\
Light 0 type\r\n0\r\n\
Light 0 body axis pos x,y,z (ft), bitmap radius (ft)\r\n-2.5,1.75,8.5,1.25\r\n\
Light 0 heading (rad), pitch (rad), heading spin speed (rad/sec)\r\n0.000000,-0.174533,0.000000\r\n\
Light 0 cone: length (ft), base radius (ft), rim radius (ft), texture name\r\n75.000000,0.700000,11.000000,litefuzz.raw\r\n\
Light 0 source: bitmap name\r\nheadlite.raw\r\n\
Light 0 ms on, ms off (0 if light doesn't blink)\r\n0,0\r\n\
Light 1 type\r\n4\r\n\
Light 1 body axis pos x,y,z (ft), bitmap radius (ft)\r\n4.2,2.6,5.0,0.7\r\n\
Light 1 heading (rad), pitch (rad), heading spin speed (rad/sec)\r\n1.570796,0.000000,0.000000\r\n\
Light 1 cone: length (ft), base radius (ft), rim radius (ft), texture name\r\n0.000000,0.000000,0.000000,redfuzz.raw\r\n\
Light 1 source: bitmap name\r\nbraklite.raw\r\n\
Light 1 ms on, ms off (0 if light doesn't blink)\r\n300,700\r\n";

    #[test]
    fn reads_the_parts_and_where_the_tires_go() {
        let truck = TruckFile::parse(TRUCK).unwrap();
        assert_eq!(truck.name, "Test Foot");
        assert_eq!(truck.body_model, "foot.bin");
        // Written without its extension.
        assert_eq!(truck.axle_model, "axle3.bin");
        assert_eq!(truck.tires.front_right, [4.5, -3.5, 6.5]);
        assert_eq!(truck.tires.front_left, [-4.5, -3.5, 6.5]);
        assert_eq!(truck.tires.rear_right, [4.25, -3.75, -6.25]);
        assert_eq!(truck.tires.rear_left, [-4.25, -3.75, -6.25]);
        assert_eq!(truck.scrape_points, [[-3.75, 0.0, 9.5], [3.75, 2.5, -9.5]]);
        assert_eq!(truck.axle_bar_offset, Some([1.25, -2.5, 0.0]));
        assert_eq!(truck.bar_texture, None);
        assert_eq!(truck.shock_texture.as_deref(), Some("SILVER.RAW"));
        assert_eq!(truck.driveshaft, None);
        assert_eq!(truck.upper_axle_bar_offset, None);
        assert_eq!(truck.instrument_cluster.as_deref(), Some("powerbig"));
    }

    #[test]
    fn reads_the_lights() {
        let truck = TruckFile::parse(TRUCK).unwrap();
        assert_eq!(truck.lights.len(), 2);
        let head = &truck.lights[0];
        assert_eq!(head.kind, 0);
        assert_eq!(head.position, [-2.5, 1.75, 8.5]);
        assert_eq!(head.bitmap_radius, 1.25);
        assert_eq!([head.heading, head.pitch, head.spin], [0.0, -0.174533, 0.0]);
        assert_eq!(
            [
                head.cone_length,
                head.cone_base_radius,
                head.cone_rim_radius
            ],
            [75.0, 0.7, 11.0]
        );
        assert_eq!(head.cone_texture, "litefuzz.raw");
        assert_eq!(head.bitmap, "headlite.raw");
        assert_eq!(head.blink_ms, [0, 0]);
        assert_eq!(truck.lights[1].blink_ms, [300, 700]);
        assert_eq!(truck.lights[1].kind, 4);
    }

    #[test]
    fn a_light_that_cannot_be_read_is_left_out() {
        let broken = TRUCK.replace("4.2,2.6,5.0,0.7", "4.2,somewhere");
        let truck = TruckFile::parse(&broken).unwrap();
        assert_eq!(truck.lights.len(), 1);
        assert_eq!(truck.lights[0].bitmap, "headlite.raw");
        let none = TRUCK.replace("Number of Lights", "Number of Lamps");
        assert!(TruckFile::parse(&none).unwrap().lights.is_empty());
    }

    #[test]
    fn reads_where_the_driveshaft_meets_the_body_unless_it_is_nowhere() {
        let with = TRUCK.replace(
            "axlebarOffset",
            "driveshaftPos\r\n0.0,-2.3,-0.125\r\naxlebarOffset",
        );
        assert_eq!(
            TruckFile::parse(&with).unwrap().driveshaft,
            Some([0.0, -2.3, -0.125])
        );
        let none = TRUCK.replace(
            "axlebarOffset",
            "driveshaftPos\r\n0.000000,0.000000,0.000000\r\naxlebarOffset",
        );
        assert_eq!(TruckFile::parse(&none).unwrap().driveshaft, None);
    }

    #[test]
    fn axle_bars_far_away_mean_none() {
        let car = TRUCK.replace("1.25,-2.5,0.0", "-2.000000,999.000000,0.000000");
        assert_eq!(TruckFile::parse(&car).unwrap().axle_bar_offset, None);
    }

    #[test]
    fn reads_the_name_alone_from_a_file_with_nothing_else_right() {
        assert_eq!(TruckFile::parse_name(TRUCK).unwrap(), "Test Foot");
        assert_eq!(
            TruckFile::parse_name("MTM2 truckName\r\nHalf a Truck\r\ngarbage").unwrap(),
            "Half a Truck"
        );
        assert!(TruckFile::parse_name("hills.lvl\r\n").is_err());
        assert!(TruckFile::parse_name("").is_err());
    }

    #[test]
    fn names_the_tire_models_finest_first() {
        let truck = TruckFile::parse(TRUCK).unwrap();
        assert_eq!(
            truck.tire_model_names(Side::Left),
            ["FOOT_16L.BIN", "FOOT_10L.BIN", "FOOT_08L.BIN"]
        );
        assert_eq!(truck.tire_model_names(Side::Right)[0], "FOOT_16R.BIN");
        // Not for an MTM2 truck, whatever models the archive holds.
        assert_eq!(truck.wheel_tire_model_name(Wheel::RearLeft), None);
    }

    #[test]
    fn reads_an_mtm2_1_truck_file_and_names_a_tire_for_each_wheel() {
        let extended = TRUCK.replace("MTM2 truckName", "MTM2.1 truckName")
            + "superiorAxlebarOffset\r\n200.000000,200.000000,400.000000\r\n";
        let truck = TruckFile::parse(&extended).unwrap();
        assert!(truck.extended);
        assert_eq!(truck.name, "Test Foot");
        assert_eq!(truck.upper_axle_bar_offset, Some([200.0, 200.0, 400.0]));
        assert_eq!(truck.tires.rear_left, [-4.25, -3.75, -6.25]);
        assert_eq!(TruckFile::parse_name(&extended).unwrap(), "Test Foot");
        assert_eq!(
            [
                Wheel::FrontLeft,
                Wheel::FrontRight,
                Wheel::RearLeft,
                Wheel::RearRight
            ]
            .map(|wheel| truck.wheel_tire_model_name(wheel).unwrap()),
            [
                "FOOT_16FL.BIN",
                "FOOT_16FR.BIN",
                "FOOT_16RL.BIN",
                "FOOT_16RR.BIN"
            ]
        );
        assert!(!TruckFile::parse(TRUCK).unwrap().extended);
        // The second set is for MTM2.1 trucks only.
        let classic = extended.replace("MTM2.1 truckName", "MTM2 truckName");
        assert_eq!(
            TruckFile::parse(&classic).unwrap().upper_axle_bar_offset,
            None
        );
    }

    #[test]
    fn reports_what_is_wrong_and_where() {
        let broken = TRUCK.replace(
            "raxle.rtire.static_bpos.y\r\n-3.75",
            "raxle.rtire.static_bpos.y\r\nlow",
        );
        let error = TruckFile::parse(&broken).unwrap_err();
        let PodError::BadValue { field, line, text } = error else {
            panic!("wrong error: {error:?}");
        };
        assert_eq!(field, "raxle.rtire.static_bpos.y");
        assert_eq!(text, "low");
        assert_eq!(broken.lines().nth(line - 1).unwrap().trim(), "low");

        let no_axle = TRUCK.replace("axleModelName", "axelModelName");
        assert!(matches!(
            TruckFile::parse(&no_axle),
            Err(PodError::MissingField { field }) if field == "axleModelName"
        ));
        // A track file, say, or a truck from another game.
        assert!(matches!(
            TruckFile::parse("truckName\nOld Foot\n"),
            Err(PodError::Unsupported(_))
        ));
        assert!(matches!(
            TruckFile::parse(""),
            Err(PodError::Unsupported(_))
        ));
    }
}
