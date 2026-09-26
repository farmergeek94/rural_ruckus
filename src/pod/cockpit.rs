//! The cockpit layout (`DATA\POWERBIG.480` and the like): where the dashboard's pictures,
//! dials, steering wheel and mirror go on the screen. See `docs/formats/cockpit.md`.
//!
//! The file describes itself: each value follows a `;` line that names it. The fields are
//! found by those names, not by their places, so that a file with a field missing or out
//! of order still gives the rest. A field that is missing or cannot be read is `None`.
//! Positions and sizes stay in the file's own pixels, on the screen the file is made for
//! (640 x 480 for a `.480` file), with y down from the top.

use super::PodError;

/// The palette index that is not drawn in every cockpit picture: where the 3D view shows
/// through the background, and round the steering wheel. **Measured** on the base game's
/// `COCKPIT.POD`: in each of the four backgrounds of `POWERBIG.480`, every pixel of index 0
/// is inside the 3D window, and index 0 is the only black in their palettes.
pub const SEE_THROUGH_INDEX: u8 = 0;

/// The extensions a layout file has, one for each size of screen, best first, and each
/// one's screen in pixels across and down. **Measured**: `POWERBIG.480`'s backgrounds are
/// 640 x 480, `.400`'s 320 x 400 and `.200`'s 320 x 200 (bytes, and a 3D window as wide as
/// the picture).
pub const SCREENS: [(&str, [u32; 2]); 3] = [
    ("480", [640, 480]),
    ("400", [320, 400]),
    ("200", [320, 200]),
];

/// Everything a cockpit layout file says, in its own units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CockpitLayout {
    /// The file names of the background pictures, as written: looking ahead, left, right
    /// and back. **Measured** by their names (`pbig480.raw`, `pbigl480.raw`,
    /// `pbigr480.raw`, `pbigb480.raw`) and by what each shows.
    pub backgrounds: Vec<String>,
    /// Where the 3D view is drawn: left, top, width and height, in pixels.
    pub window: Option<[f32; 4]>,
    pub speedometer: Gauge,
    pub tachometer: Gauge,
    /// Where the steering wheel's picture goes: left, top, width and height.
    pub steering_wheel: Option<[f32; 4]>,
    /// Four numbers under `Erase window`. What they mean is **open**.
    pub erase_window: Option<[f32; 4]>,
    /// What the file names of the steering wheel's pictures begin with (`PW480`).
    pub steering_wheel_base: Option<String>,
    pub mirrors: Vec<Mirror>,
    /// What the file names of the gear shifter's pictures begin with (`ps480`).
    pub shifter_base: Option<String>,
    /// Where the shifter's picture goes: left, top, width and height.
    pub shifter: Option<[f32; 4]>,
    pub shift_light_bitmap: Option<String>,
    /// Where the shift light's picture goes: left, top, width and height.
    pub shift_light: Option<[f32; 4]>,
}

/// A dial with a needle. The angles are in degrees, clockwise from straight up on the
/// screen: the needle points at `zero_angle + degrees_per_unit * value`. **Measured**: the
/// speedometer's marks for 10 to 100 mph in `PBIG480.RAW` are within 3 degrees of that.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gauge {
    /// Where the needle turns, in pixels.
    pub center: Option<[f32; 2]>,
    /// How long the needle is, in pixels.
    pub radius: Option<f32>,
    /// The needle's model (.BIN), as written.
    pub needle_model: Option<String>,
    pub zero_angle: Option<f32>,
    /// Degrees per mile per hour on the speedometer, per revolution a minute on the
    /// tachometer.
    pub degrees_per_unit: Option<f32>,
    /// A rectangle round part of the dial: left, top, width and height. What the game
    /// redrew there is **open**.
    pub face_redraw: Option<[f32; 4]>,
}

/// A rear-view mirror: a second view drawn into a rectangle of the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mirror {
    /// Where its view goes: left, top, width and height.
    pub view: Option<[f32; 4]>,
    /// Three numbers under `Angles` (`0,0,32768`). Which is which, and the unit, are
    /// **open**; 32768 would be half a turn in 65536ths, which a mirror that looks back needs.
    pub angles: Option<[f32; 3]>,
    /// Three numbers under `Translation`. **Open**.
    pub translation: Option<[f32; 3]>,
    /// One number under `Zoom` (49152). **Open**.
    pub zoom: Option<f32>,
    /// Where its frame picture goes: left, top, width and height.
    pub frame: Option<[f32; 4]>,
    /// Its frame picture, as written.
    pub frame_bitmap: Option<String>,
}

/// One `;` line and the value lines that follow it.
struct Field<'a> {
    /// The comment's text, lower case.
    label: String,
    values: Vec<&'a str>,
}

impl CockpitLayout {
    /// Reads a layout. Only a file that names no background at all is an error: without
    /// one there is no dashboard to draw.
    pub fn parse(text: &str) -> Result<Self, PodError> {
        let fields = fields(text);
        let find = |label: &str| fields.iter().find(|field| field.label.starts_with(label));
        let line = |label: &str| find(label).and_then(|field| field.values.first().copied());
        let name = |label: &str| line(label).map(str::to_string);
        let number = |label: &str| line(label).and_then(numbers::<1>).map(|[n]| n);
        let pair = |label: &str| line(label).and_then(numbers::<2>);
        let rect = |label: &str| line(label).and_then(numbers::<4>);

        let backgrounds: Vec<String> = find("background image file")
            .map(|field| field.values.iter().map(|name| name.to_string()).collect())
            .unwrap_or_default();
        if backgrounds.is_empty() {
            return Err(PodError::MissingField {
                field: "Background image file".into(),
            });
        }
        let gauge = |name: &str| Gauge {
            center: pair(&format!("{name} center")),
            radius: number(&format!("{name} radius")),
            needle_model: line(&format!("{name} needle model")).map(str::to_string),
            zero_angle: number(&format!("{name} zero angle")),
            degrees_per_unit: number(&format!("{name} degrees per")),
            face_redraw: rect(&format!("{name} face redraw")),
        };

        Ok(Self {
            backgrounds,
            window: rect("3d window"),
            speedometer: gauge("speedometer"),
            tachometer: gauge("tachometer"),
            steering_wheel: rect("steering wheel coordinate"),
            erase_window: rect("erase window"),
            steering_wheel_base: name("steering wheel base filename"),
            mirrors: mirrors(&fields),
            shifter_base: name("shifter name"),
            shifter: rect("shifter position"),
            shift_light_bitmap: name("shift light bitmap"),
            shift_light: rect("shift light position"),
        })
    }
}

/// The file cut at its `;` lines. Blank lines are skipped; lines before the first `;`
/// line belong to no field.
fn fields(text: &str) -> Vec<Field<'_>> {
    let mut fields: Vec<Field> = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(label) = line.strip_prefix(';') {
            fields.push(Field {
                label: label.trim().to_ascii_lowercase(),
                values: Vec::new(),
            });
        } else if let Some(field) = fields.last_mut() {
            field.values.push(line);
        }
    }
    fields
}

/// The mirrors, as many as `Number of mirrors` says, each from its `Mirror Location` line
/// up to the next one. The fields of a mirror are named alike in every mirror, so they
/// are told apart by where they are.
fn mirrors(fields: &[Field]) -> Vec<Mirror> {
    let count = fields
        .iter()
        .find(|field| field.label.starts_with("number of mirrors"))
        .and_then(|field| field.values.first())
        .and_then(|text| text.parse::<usize>().ok())
        .unwrap_or(0);
    let mut mirrors: Vec<Mirror> = Vec::new();
    for field in fields {
        let value = field.values.first().copied().unwrap_or("");
        if field.label.starts_with("mirror location") {
            if mirrors.len() == count {
                break;
            }
            mirrors.push(Mirror {
                view: numbers(value),
                ..Mirror::default()
            });
            continue;
        }
        let Some(mirror) = mirrors.last_mut() else {
            continue;
        };
        if field.label.starts_with("angles") {
            mirror.angles = numbers(value);
        } else if field.label.starts_with("translation") {
            mirror.translation = numbers(value);
        } else if field.label.starts_with("zoom") {
            mirror.zoom = numbers::<1>(value).map(|[zoom]| zoom);
        } else if field.label.starts_with("bitmap position") {
            mirror.frame = numbers(value);
        } else if field.label.starts_with("bitmap name") {
            mirror.frame_bitmap = Some(value.to_string()).filter(|name| !name.is_empty());
        }
    }
    mirrors
}

/// A line of exactly `N` comma-separated numbers.
fn numbers<const N: usize>(text: &str) -> Option<[f32; N]> {
    let values: Vec<f32> = text
        .split(',')
        .map(|value| value.trim().parse().ok())
        .collect::<Option<_>>()?;
    values.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Laid out as the base game's `POWERBIG.480` is, with CRLF line endings, a trailing
    /// blank line and notes in the labels, and made-up values.
    const LAYOUT: &str = "; Background image file\r\nfront.raw\r\nleft.raw\r\nright.raw\r\n\
back.raw\r\n; 3D Window coordinate and size\r\n0,80,640,250\r\n; Speedometer center\r\n\
150,300\r\n; Speedometer radius\r\n30\r\n; Speedometer needle model\r\nneedle.bin\r\n\
; Speedometer zero angle (was 315.0)\r\n300.5\r\n\
; Speedometer degrees per mph : 240 / 90 = 2.6667 (was 2.6)\r\n2.5\r\n\
; Speedometer face redraw coordinate and size\r\n120,290,20,40\r\n\
; Tachometer center\r\n430,300\r\n; Tachometer radius\r\n20\r\n\
; Tachometer needle model\r\nneedle.bin\r\n; Tachometer zero angle (was 157.5)\r\n150.0\r\n\
; Tachometer degrees per rpm : 235 / 9000 = 0.0261 (was 0.0257)\r\n0.025\r\n\
; Tachometer face redraw coordinate and size\r\n430,280,10,40\r\n\
; Steering wheel coordinate and size\r\n140,330,350,150\r\n; Erase window\r\n1,2,3,4\r\n\
; Steering wheel base filename\r\nWHEEL\r\n; Number of mirrors\r\n1\r\n\
; Mirror Location, size\r\n530,120,100,50\r\n; Angles\r\n0,0,32768\r\n; Translation\r\n\
0,0,0\r\n; Zoom\r\n49152\r\n; Bitmap position and size\r\n528,116,108,56\r\n\
; Bitmap name\r\nframe.raw\r\n; Shifter name\r\nshift\r\n; Shifter position and size\r\n\
496,272,128,192\r\n; Shift light bitmap\r\nlight.raw\r\n\
; Shift light position and size\r\n378,261,18,19\r\n\r\n";

    #[test]
    fn reads_every_field_by_its_label() {
        let layout = CockpitLayout::parse(LAYOUT).unwrap();
        assert_eq!(
            layout.backgrounds,
            ["front.raw", "left.raw", "right.raw", "back.raw"]
        );
        assert_eq!(layout.window, Some([0.0, 80.0, 640.0, 250.0]));
        let speedometer = &layout.speedometer;
        assert_eq!(speedometer.center, Some([150.0, 300.0]));
        assert_eq!(speedometer.radius, Some(30.0));
        assert_eq!(speedometer.needle_model.as_deref(), Some("needle.bin"));
        assert_eq!(speedometer.zero_angle, Some(300.5));
        assert_eq!(speedometer.degrees_per_unit, Some(2.5));
        assert_eq!(speedometer.face_redraw, Some([120.0, 290.0, 20.0, 40.0]));
        assert_eq!(layout.tachometer.center, Some([430.0, 300.0]));
        assert_eq!(layout.tachometer.degrees_per_unit, Some(0.025));
        assert_eq!(layout.steering_wheel, Some([140.0, 330.0, 350.0, 150.0]));
        assert_eq!(layout.erase_window, Some([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(layout.steering_wheel_base.as_deref(), Some("WHEEL"));
        assert_eq!(
            layout.mirrors,
            [Mirror {
                view: Some([530.0, 120.0, 100.0, 50.0]),
                angles: Some([0.0, 0.0, 32768.0]),
                translation: Some([0.0; 3]),
                zoom: Some(49152.0),
                frame: Some([528.0, 116.0, 108.0, 56.0]),
                frame_bitmap: Some("frame.raw".into()),
            }]
        );
        assert_eq!(layout.shifter_base.as_deref(), Some("shift"));
        assert_eq!(layout.shift_light, Some([378.0, 261.0, 18.0, 19.0]));
    }

    #[test]
    fn a_field_missing_or_bad_costs_only_that_field() {
        let text = LAYOUT
            .replace("150,300\r\n", "150;300\r\n")
            .replace("; Tachometer radius\r\n20\r\n", "")
            .replace("; Number of mirrors\r\n1", "; Number of mirrors\r\n0");
        let layout = CockpitLayout::parse(&text).unwrap();
        assert_eq!(layout.speedometer.center, None);
        assert_eq!(layout.speedometer.radius, Some(30.0));
        assert_eq!(layout.tachometer.radius, None);
        assert_eq!(layout.tachometer.center, Some([430.0, 300.0]));
        assert!(layout.mirrors.is_empty());
        assert_eq!(layout.shifter_base.as_deref(), Some("shift"));
    }

    #[test]
    fn a_layout_with_no_background_is_an_error() {
        assert!(CockpitLayout::parse("").is_err());
        assert!(CockpitLayout::parse("; Background image file\r\n; 3D Window\r\n1,2,3,4").is_err());
        assert!(CockpitLayout::parse("front.raw\r\n; Speedometer radius\r\n3").is_err());
    }
}
