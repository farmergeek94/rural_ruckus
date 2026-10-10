//! The level file (.LVL): a list of the other files that make up a track, one per line,
//! identified by position, then lighting values and, in MTM2's own levels, a labelled
//! water height. See `docs/formats/level.md`.

use super::PodError;

/// The line that comes before the water height.
const WATER_HEIGHT_LABEL: &str = "!waterHeight";

/// Units of the water height in a foot. The height is written in quarters of a height
/// step, and a step is 2 ft. **Measured**: see `docs/formats/level.md`.
const WATER_UNITS_PER_FOOT: f32 = 2.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    /// Line 2: track description text.
    pub description_file: String,
    /// Line 3: terrain heightmap.
    pub heightmap_file: String,
    /// Line 4: which texture covers each terrain cell.
    pub texture_map_file: String,
    /// Line 5: the 256-colour palette the textures use.
    pub palette_file: String,
    /// Line 6: the list of ground textures.
    pub texture_list_file: String,
    /// Lines 11 and 12: the sky's picture and its palette. `None` where the line is
    /// missing or empty. **Reference** and **measured**: see `docs/formats/level.md`.
    pub sky_file: Option<String>,
    pub sky_palette_file: Option<String>,
    /// The line after `!waterHeight`, as written: the height of the water's surface, in
    /// half feet. `None` where the level has no such line, or it isn't a number. MTM2's
    /// arenas write 0, for no water.
    pub water_height: Option<i32>,
}

impl Level {
    pub fn parse(text: &str) -> Result<Self, PodError> {
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        let line = |index: usize, field: &str| match lines.get(index) {
            Some(name) if !name.is_empty() => Ok(name.to_string()),
            _ => Err(PodError::MissingField {
                field: field.into(),
            }),
        };
        Ok(Self {
            description_file: line(1, "description file (line 2)")?,
            heightmap_file: line(2, "heightmap file (line 3)")?,
            texture_map_file: line(3, "texture map file (line 4)")?,
            palette_file: line(4, "palette file (line 5)")?,
            texture_list_file: line(5, "texture list file (line 6)")?,
            sky_file: line(10, "sky").ok(),
            sky_palette_file: line(11, "sky palette").ok(),
            water_height: lines
                .iter()
                .position(|line| line.eq_ignore_ascii_case(WATER_HEIGHT_LABEL))
                .and_then(|index| lines.get(index + 1))
                .and_then(|value| value.parse().ok()),
        })
    }

    /// The height of the water's surface, in feet, as the heightmap's are. `None` where the
    /// level gives none.
    pub fn water_feet(&self) -> Option<f32> {
        self.water_height
            .map(|height| height as f32 / WATER_UNITS_PER_FOOT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_file_names_by_line() {
        let level = Level::parse(
            "0\r\nhills.txt\r\nhills.raw\r\nhills.clr\r\nhills.act\r\nhills.tex\r\nzero.raw\r\n",
        )
        .unwrap();
        assert_eq!(level.heightmap_file, "hills.raw");
        assert_eq!(level.texture_map_file, "hills.clr");
        assert_eq!(level.texture_list_file, "hills.tex");
    }

    #[test]
    fn reads_the_sky_on_lines_11_and_12_where_there_is_one() {
        let level = Level::parse(
            "0\r\nb.txt\r\nb.raw\r\nb.clr\r\nb.act\r\nb.tex\r\nzero.raw\r\nb.pup\r\nb.ani\r\n\
             b.tdf\r\ncloudy2.raw\r\ncloudy2.act\r\n",
        )
        .unwrap();
        assert_eq!(level.sky_file.as_deref(), Some("cloudy2.raw"));
        assert_eq!(level.sky_palette_file.as_deref(), Some("cloudy2.act"));
        let short = Level::parse("0\nb.txt\nb.raw\nb.clr\nb.act\nb.tex\n").unwrap();
        assert_eq!(short.sky_file, None);
    }

    #[test]
    fn reads_the_water_height_after_its_label() {
        let level = Level::parse(
            "0\r\nb.txt\r\nb.raw\r\nb.clr\r\nb.act\r\nb.tex\r\n255\r\n!waterHeight\r\n252\r\n",
        )
        .unwrap();
        assert_eq!(level.water_height, Some(252));
        assert_eq!(level.water_feet(), Some(126.0));
    }

    #[test]
    fn a_level_without_water_says_so() {
        let level = Level::parse("0\nb.txt\nb.raw\nb.clr\nb.act\nb.tex\n255\n\n\u{1a}\n").unwrap();
        assert_eq!(level.water_height, None);
        // A label with nothing readable after it is no water either.
        let level = Level::parse("0\nb.txt\nb.raw\nb.clr\nb.act\nb.tex\n!waterHeight\n").unwrap();
        assert_eq!(level.water_feet(), None);
    }

    #[test]
    fn a_short_file_is_an_error() {
        let error = Level::parse("0\nhills.txt\n").unwrap_err();
        assert!(matches!(error, PodError::MissingField { .. }));
    }
}
