//! Texture types (.TTY): a number for each of a track's textures, which says what kind of
//! surface it is. See `docs/formats/texture_types.md`.
//!
//! Only the numbers are read here. What a number means to the game is the caller's
//! business, and mostly not known.

use super::PodError;

/// A track's texture types, in the file's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextureTypes(Vec<TextureType>);

/// One line of the file: a texture and its type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureType {
    /// A texture file name, as written. It may be empty.
    pub texture: String,
    /// The texture's type, as written.
    pub class: u32,
}

impl TextureTypes {
    /// Text: a count on the first line, then one `NAME.RAW,number` line per texture.
    pub fn parse(text: &str) -> Result<Self, PodError> {
        let mut lines = text.lines().map(str::trim);
        let count_line = lines.next().unwrap_or("");
        let count: usize = count_line.parse().map_err(|_| PodError::BadValue {
            field: "texture type count".into(),
            line: 1,
            text: count_line.to_string(),
        })?;

        let mut types = Vec::with_capacity(count);
        for (index, line) in lines.enumerate().filter(|(_, line)| !line.is_empty()) {
            if types.len() == count {
                break;
            }
            let bad = || PodError::BadValue {
                field: "a texture name and type".into(),
                line: index + 2,
                text: line.to_string(),
            };
            let (texture, class) = line.rsplit_once(',').ok_or_else(bad)?;
            types.push(TextureType {
                texture: texture.trim().to_string(),
                class: class.trim().parse().map_err(|_| bad())?,
            });
        }
        if types.len() < count {
            return Err(PodError::MissingField {
                field: format!("texture type {} of {count}", types.len() + 1),
            });
        }
        Ok(Self(types))
    }

    pub fn entries(&self) -> &[TextureType] {
        &self.0
    }

    /// The type of a texture, ignoring case. A texture listed more than once takes its
    /// first type: which one the game takes is not known.
    pub fn class_of(&self, texture: &str) -> Option<u32> {
        self.0
            .iter()
            .find(|entry| entry.texture.eq_ignore_ascii_case(texture))
            .map(|entry| entry.class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_texture_and_its_type() {
        let types =
            TextureTypes::parse("3\r\nGOLFGRAS.RAW,608\r\n,0\r\n11EI4.RAW,901\r\n").unwrap();
        assert_eq!(types.entries().len(), 3);
        assert_eq!(types.entries()[1].texture, "");
        assert_eq!(types.class_of("golfgras.raw"), Some(608));
        assert_eq!(types.class_of("11EI4.RAW"), Some(901));
        assert_eq!(types.class_of("TRAC2.RAW"), None);
    }

    #[test]
    fn an_empty_list_is_read() {
        assert!(TextureTypes::parse("0\r\n").unwrap().entries().is_empty());
    }

    #[test]
    fn a_texture_listed_twice_takes_its_first_type() {
        let types = TextureTypes::parse("2\n88CRK1.RAW,318\n88CRK1.RAW,320\n").unwrap();
        assert_eq!(types.class_of("88CRK1.RAW"), Some(318));
    }

    #[test]
    fn a_short_headless_or_garbled_list_is_an_error() {
        assert!(matches!(
            TextureTypes::parse("2\nA.RAW,101\n"),
            Err(PodError::MissingField { .. })
        ));
        assert!(matches!(
            TextureTypes::parse("A.RAW,101\n"),
            Err(PodError::BadValue { line: 1, .. })
        ));
        assert!(matches!(
            TextureTypes::parse("2\nA.RAW,101\nB.RAW\n"),
            Err(PodError::BadValue { line: 3, .. })
        ));
        assert!(matches!(
            TextureTypes::parse("1\nA.RAW,ice\n"),
            Err(PodError::BadValue { line: 2, .. })
        ));
    }
}
