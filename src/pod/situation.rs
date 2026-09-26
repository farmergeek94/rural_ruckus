//! The track file (.SIT, "situation"): a saved game state that doubles as the track
//! definition. See `docs/formats/situation.md`.
//!
//! It is text. Most values sit on the line after a label line that names them, such as
//! `ipos` followed by `4352.6,135.9,3948.7`, and sections open with a `*** Title ***`
//! line. Only what the game uses so far is read; everything else is skipped, which also
//! makes the parser indifferent to fields it doesn't know.

use super::PodError;

/// Values of `SituationBox::kind`.
pub mod box_type {
    /// Not solid: driving through it passes the checkpoint. Checkpoints count in the
    /// order they appear in the file.
    pub const CHECKPOINT: i32 = 6;
    /// Moves along its `velocity` ("moving - use bvel" in the Traxx editor's notes).
    pub const MOVING: i32 = 10;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Situation {
    /// File name of the level (.LVL) this track uses.
    pub level_file: String,
    pub name: String,
    /// The starting grid, pole position first.
    pub vehicles: Vec<Vehicle>,
    /// Scenery, obstacles and checkpoints.
    pub boxes: Vec<SituationBox>,
    /// The first course that computer trucks follow: straight pieces, without the
    /// corners that join them.
    pub course: Vec<CourseSegment>,
    /// File names of the models drawn round the horizon (.BIN), from the Backdrop section,
    /// in the file's order. Empty for a track that has none.
    pub backdrops: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Vehicle {
    pub truck_file: String,
    /// In feet.
    pub position: [f32; 3],
    /// Heading in radians. At 0 the truck faces +Z.
    pub psi: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SituationBox {
    /// In feet.
    pub position: [f32; 3],
    /// Pitch, roll and heading in radians, as written: `theta`, `phi`, `psi`.
    pub angles: [f32; 3],
    pub shape: BoxShape,
    /// See `box_type`.
    pub kind: i32,
    pub flags: i32,
    /// In slugs (pounds over g), as written. 0 is a box that nothing can move. Absent in
    /// some files, and then 0.
    pub mass: f32,
    /// `bvel`: how fast a `box_type::MOVING` box goes, in feet per second, along the
    /// world's axes, as written. Absent in some files, and then 0.
    pub velocity: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoxShape {
    /// File name of a 3D model (.BIN).
    Model(String),
    /// An invisible box with no model: its `length,width,height`, in feet.
    Dimensions([f32; 3]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CourseSegment {
    /// In feet.
    pub start: [f32; 3],
    /// In feet.
    pub end: [f32; 3],
}

impl Situation {
    /// The track's name and nothing else: what a list of tracks needs. Empty for a track
    /// that doesn't give one.
    pub fn parse_name(text: &str) -> String {
        let lines = Lines::new(text);
        lines
            .after("Race Track Name", 0, lines.len())
            .map(|index| lines.text(index).to_string())
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Result<Self, PodError> {
        let lines = Lines::new(text);
        let end = lines.len();

        let vehicles_start = lines.section("Vehicles").unwrap_or(end);
        let boxes_start = lines.section("Boxes").unwrap_or(end);
        let course_start = lines.section("Course").unwrap_or(end);
        let vehicles_end = lines.next_section(vehicles_start);
        let boxes_end = lines.next_section(boxes_start);
        // The extra courses that follow the first repeat its labels.
        let course_end = (course_start..end)
            .find(|&index| lines.text(index).contains("Extended Course Definitions"))
            .unwrap_or_else(|| lines.next_section(course_start));

        let mut vehicles = Vec::new();
        for start in lines.labelled("truckFile", vehicles_start, vehicles_end) {
            let block_end = lines
                .labelled("truckFile", start + 1, vehicles_end)
                .next()
                .unwrap_or(vehicles_end);
            vehicles.push(Vehicle {
                truck_file: lines.text(start + 1).to_string(),
                position: lines.floats_after("ipos", start, block_end)?,
                psi: lines.floats_after::<3>("theta,phi,psi", start, block_end)?[2],
            });
        }

        let mut boxes = Vec::new();
        for start in lines.labelled("ipos", boxes_start, boxes_end) {
            let block_end = lines
                .labelled("ipos", start + 1, boxes_end)
                .next()
                .unwrap_or(boxes_end);
            let shape = match lines.after("model", start, block_end) {
                Ok(model_line) => BoxShape::Model(lines.text(model_line).to_string()),
                Err(_) => BoxShape::Dimensions(lines.floats_after(
                    "length,width,height",
                    start,
                    block_end,
                )?),
            };
            let [kind, flags] = lines.floats_after("type,flags", start, block_end)?;
            boxes.push(SituationBox {
                position: lines.floats_after("ipos", start, block_end)?,
                angles: lines.floats_after("theta,phi,psi", start, block_end)?,
                shape,
                kind: kind as i32,
                flags: flags as i32,
                mass: lines.floats_or_zero::<1>("mass", start, block_end)?[0],
                velocity: lines.floats_or_zero("bvel", start, block_end)?,
            });
        }

        let mut course = Vec::new();
        for start in lines.labelled("cstart", course_start, course_end) {
            course.push(CourseSegment {
                start: lines.floats("cstart", start + 1)?,
                end: lines.floats_after("cend", start, course_end)?,
            });
        }

        let backdrops = lines
            .section("Backdrop")
            .map_or_else(Vec::new, |start| backdrops(&lines, start));

        Ok(Self {
            level_file: lines.text(0).to_string(),
            name: lines
                .after("Race Track Name", 0, end)
                .map(|index| lines.text(index).to_string())
                .unwrap_or_default(),
            vehicles,
            boxes,
            course,
            backdrops,
        })
    }

    /// The checkpoints, in the order they must be passed.
    pub fn checkpoints(&self) -> impl Iterator<Item = &SituationBox> {
        self.boxes
            .iter()
            .filter(|situation_box| situation_box.kind == box_type::CHECKPOINT)
    }
}

/// The Backdrop section, which opens at line `start`: a count after
/// `backdropType,backdropCount`, and that many model names on the lines after
/// `backdropModelName`. A section that can't be read costs the track its backdrop, not the
/// race.
fn backdrops(lines: &Lines, start: usize) -> Vec<String> {
    let end = lines.next_section(start);
    let Ok([_, count]) = lines.floats_after::<2>("backdropType,backdropCount", start, end) else {
        return Vec::new();
    };
    let Ok(first) = lines.after("backdropModelName", start, end) else {
        return Vec::new();
    };
    (first..end)
        .take(count.max(0.0) as usize)
        .map(|index| lines.text(index))
        .take_while(|name| !name.is_empty() && !name.starts_with("***"))
        .map(str::to_string)
        .collect()
}

/// The sections of a track file, in the order they appear.
const SECTIONS: [&str; 8] = [
    "Vehicles",
    "Ramps",
    "Boxes",
    "Cylinders",
    "Top Crush",
    "Course",
    "Stadium",
    "Backdrop",
];

/// A text file as trimmed lines, in which a value sits on the line after its label. The
/// truck file (.TRK) is laid out the same way and reads itself with this too.
pub(super) struct Lines<'a>(Vec<&'a str>);

impl<'a> Lines<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Self(text.lines().map(str::trim).collect())
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    /// Line `index`, or nothing past the end of the file.
    pub(super) fn text(&self, index: usize) -> &'a str {
        self.0.get(index).copied().unwrap_or("")
    }

    /// Label lines often start with a marker character, as in `!type,flags`.
    pub(super) fn label(&self, index: usize) -> &'a str {
        self.text(index)
            .trim_start_matches(['!', '@', '$', '^', '&'])
            .trim()
    }

    /// A `*** Title ***` line that opens one of the file's sections, giving the title.
    /// Vehicles have `***Lap time***` lines of their own, which are not sections.
    fn section_title(&self, index: usize) -> Option<&'a str> {
        let line = self.text(index);
        let title = line.trim_matches('*').trim();
        (line.starts_with("***") && line.ends_with("***") && SECTIONS.contains(&title))
            .then_some(title)
    }

    fn section(&self, title: &str) -> Option<usize> {
        (0..self.len()).find(|&index| self.section_title(index) == Some(title))
    }

    /// Index of the first section heading after `start`, or the end of the file.
    fn next_section(&self, start: usize) -> usize {
        (start + 1..self.len())
            .find(|&index| self.section_title(index).is_some())
            .unwrap_or(self.len())
    }

    /// Indices of the lines in a range that carry this label.
    pub(super) fn labelled(
        &self,
        label: &'a str,
        start: usize,
        end: usize,
    ) -> impl Iterator<Item = usize> {
        (start..end.min(self.len())).filter(move |&index| self.label(index) == label)
    }

    /// Index of the value line that follows the first `label` in a range.
    pub(super) fn after(
        &self,
        label: &'a str,
        start: usize,
        end: usize,
    ) -> Result<usize, PodError> {
        self.labelled(label, start, end)
            .next()
            .map(|index| index + 1)
            .ok_or_else(|| PodError::MissingField {
                field: label.to_string(),
            })
    }

    pub(super) fn floats_after<const N: usize>(
        &self,
        label: &'a str,
        start: usize,
        end: usize,
    ) -> Result<[f32; N], PodError> {
        self.floats(label, self.after(label, start, end)?)
    }

    /// As `floats_after`, but zeros where the label isn't there at all.
    pub(super) fn floats_or_zero<const N: usize>(
        &self,
        label: &'a str,
        start: usize,
        end: usize,
    ) -> Result<[f32; N], PodError> {
        match self.after(label, start, end) {
            Ok(index) => self.floats(label, index),
            Err(_) => Ok([0.0; N]),
        }
    }

    /// Line `index` as exactly `N` comma-separated numbers.
    pub(super) fn floats<const N: usize>(
        &self,
        field: &str,
        index: usize,
    ) -> Result<[f32; N], PodError> {
        let bad_value = || PodError::BadValue {
            field: field.to_string(),
            line: index + 1,
            text: self.text(index).to_string(),
        };
        let values: Vec<f32> = self
            .text(index)
            .split(',')
            .map(|value| value.trim().parse())
            .collect::<Result<_, _>>()
            .map_err(|_| bad_value())?;
        values.try_into().map_err(|_| bad_value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cut down from a real track, keeping its layout: marker characters, CRLF line
    /// endings, a decoy truck before the vehicles section, and a second course.
    const TRACK: &str = "hills.lvl\r\n!Race Track Name\r\nTest Hills\r\nRace Track Locale\r\nNowhere\r\n\
*** Your Truck (Not used anymore) ***\r\n*********************************************\r\n\
truckFile\r\nbigfoot.trk\r\nipos\r\n1.0,2.0,3.0\r\ntheta,phi,psi\r\n0.0,0.0,9.0\r\n\
*** Vehicles ***\r\n2\r\n*********************************************\r\n\
truckFile\r\nbigfoot.trk\r\nipos\r\n4338.25,126.0,3887.125\r\nbvel\r\n0.0,0.0,0.0\r\ntheta,phi,psi\r\n0.0,0.0,6.308113\r\n\
***Lap time***\r\n0.0\r\n\
*********************************************\r\n\
truckFile\r\ncrusher.trk\r\nipos\r\n4368.0,126.0,3886.5\r\ntheta,phi,psi\r\n0.0,0.0,6.25\r\n\
*** Ramps ***\r\n0\r\n*** Boxes ***\r\n3\r\n\
*********************************************\r\n\
ipos\r\n3862.5,140.0,4214.25\r\ntheta,phi,psi\r\n0.0,0.0,15.75\r\nmodel\r\nC8CHEK1L.BIN\r\nmass\r\n0.0\r\n!type,flags\r\n7,0\r\n\
*********************************************\r\n\
ipos\r\n3809.0,139.0,4214.5\r\ntheta,phi,psi\r\n0.0,0.0,13.75\r\nlength,width,height\r\n2.0,2.0,38.0\r\nmass\r\n0.777025\r\n\
bvel\r\n0.00,0.00,-170.00\r\np,q,r\r\n0.0,0.0,0.0\r\n!type,flags\r\n11,0\r\npriority\r\n0\r\n\
*********************************************\r\n\
ipos\r\n3837.5,136.0,4197.0\r\ntheta,phi,psi\r\n0.1,0.2,15.5\r\nmodel\r\nCKBOXN.BIN\r\n!type,flags\r\n6,1\r\n\
*** Cylinders ***\r\n0\r\n*** Course ***\r\nc1Count,course_direction\r\n1,0\r\n\
********************************************* 1\r\nctype,cspeed_type\r\n1,0\r\n\
cstart\r\n4348.0,120.0,3940.5\r\ncend\r\n4348.0,120.0,4203.5\r\n\
@*********** Extended Course Definitions *************\r\n1\r\n[Course 1] c1Count,course_direction\r\n1,0\r\n\
cstart\r\n1.0,1.0,1.0\r\ncend\r\n2.0,2.0,2.0\r\n*** Stadium ***\r\n\
!stadiumFlag,x,z,sx,sz,stadiumModelName\r\n0,64,64,8,10,none\r\n\
*** Backdrop ***\r\nbackdropType,backdropCount\r\n0,2\r\nbackdropModelName\r\n\
cs4drop1.bin\r\ncs4drop2.bin\r\n";

    #[test]
    fn reads_the_name_alone() {
        assert_eq!(Situation::parse_name(TRACK), "Test Hills");
        // A track needn't give one, and what follows the name needn't be readable.
        assert_eq!(Situation::parse_name("hills.lvl\r\n"), "");
        assert_eq!(
            Situation::parse_name("x.lvl\r\nRace Track Name\r\nHalf a Track\r\nipos\r\n"),
            "Half a Track"
        );
    }

    #[test]
    fn reads_the_backdrop_models() {
        let situation = Situation::parse(TRACK).unwrap();
        assert_eq!(situation.backdrops, ["cs4drop1.bin", "cs4drop2.bin"]);
    }

    #[test]
    fn a_track_without_a_backdrop_has_none() {
        let none = TRACK.replace("0,2\r\nbackdropModelName", "0,0\r\nbackdropModelName");
        assert!(Situation::parse(&none).unwrap().backdrops.is_empty());
        // Nor does one whose section is cut short.
        let cut = &TRACK[..TRACK.find("backdropModelName").unwrap()];
        assert!(Situation::parse(cut).unwrap().backdrops.is_empty());
    }

    #[test]
    fn reads_names_and_the_starting_grid() {
        let situation = Situation::parse(TRACK).unwrap();
        assert_eq!(situation.level_file, "hills.lvl");
        assert_eq!(situation.name, "Test Hills");

        // The unused truck above the vehicles section isn't on the grid.
        assert_eq!(situation.vehicles.len(), 2);
        assert_eq!(situation.vehicles[0].truck_file, "bigfoot.trk");
        assert_eq!(situation.vehicles[0].position, [4338.25, 126.0, 3887.125]);
        assert_eq!(situation.vehicles[0].psi, 6.308113);
        assert_eq!(situation.vehicles[1].truck_file, "crusher.trk");
        assert_eq!(situation.vehicles[1].psi, 6.25);
    }

    #[test]
    fn reads_boxes_with_models_and_with_dimensions() {
        let situation = Situation::parse(TRACK).unwrap();
        assert_eq!(situation.boxes.len(), 3);
        assert_eq!(
            situation.boxes[0].shape,
            BoxShape::Model("C8CHEK1L.BIN".into())
        );
        assert_eq!(situation.boxes[0].kind, 7);
        assert_eq!(
            situation.boxes[1].shape,
            BoxShape::Dimensions([2.0, 2.0, 38.0])
        );
        assert_eq!(situation.boxes[1].kind, 11);

        // Mass and velocity as written, and zero where a box has none.
        assert_eq!(situation.boxes[0].mass, 0.0);
        assert_eq!(situation.boxes[1].mass, 0.777025);
        assert_eq!(situation.boxes[1].velocity, [0.0, 0.0, -170.0]);
        assert_eq!(situation.boxes[2].mass, 0.0);
        assert_eq!(situation.boxes[2].velocity, [0.0; 3]);

        let checkpoints: Vec<_> = situation.checkpoints().collect();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].position, [3837.5, 136.0, 4197.0]);
        assert_eq!(checkpoints[0].angles, [0.1, 0.2, 15.5]);
        assert_eq!(checkpoints[0].flags, 1);
    }

    #[test]
    fn reads_only_the_first_course() {
        let situation = Situation::parse(TRACK).unwrap();
        assert_eq!(
            situation.course,
            [CourseSegment {
                start: [4348.0, 120.0, 3940.5],
                end: [4348.0, 120.0, 4203.5],
            }]
        );
    }

    #[test]
    fn reports_the_line_of_a_bad_value() {
        let broken = TRACK.replace("4368.0,126.0,3886.5", "4368.0,oops,3886.5");
        let error = Situation::parse(&broken).unwrap_err();
        let PodError::BadValue { field, line, text } = error else {
            panic!("wrong error: {error:?}");
        };
        assert_eq!(field, "ipos");
        assert_eq!(text, "4368.0,oops,3886.5");
        assert_eq!(broken.lines().nth(line - 1).unwrap().trim(), text);
    }

    #[test]
    fn a_file_with_no_sections_is_an_empty_track() {
        let situation = Situation::parse("hills.lvl\n").unwrap();
        assert_eq!(situation.level_file, "hills.lvl");
        assert!(situation.vehicles.is_empty());
        assert!(situation.boxes.is_empty());
        assert!(situation.course.is_empty());
        assert!(Situation::parse("").unwrap().level_file.is_empty());
    }
}
