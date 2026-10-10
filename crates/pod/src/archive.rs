//! The POD archive itself (the first-generation format, which has no magic number).
//! See `docs/formats/pod.md`.

use super::PodError;

const COMMENT_LENGTH: usize = 80;
const NAME_LENGTH: usize = 32;
/// The length of an archive's header: its file count and comment, in bytes.
pub const HEADER_LENGTH: usize = 4 + COMMENT_LENGTH;
const ENTRY_LENGTH: usize = NAME_LENGTH + 4 + 4;

#[derive(Clone, Debug)]
pub struct PodArchive {
    comment: String,
    entries: Vec<Entry>,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Path inside the archive as written, such as `DATA\NUTRACK.RAW`: backslashes,
    /// usually upper case.
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

impl Entry {
    /// The name without its folders.
    pub fn file_name(&self) -> &str {
        self.name.rsplit(['\\', '/']).next().unwrap_or(&self.name)
    }

    /// The folders before the name, such as `DATA`, or nothing.
    pub fn folder(&self) -> &str {
        self.name
            .rsplit_once(['\\', '/'])
            .map_or("", |(folder, _)| folder)
    }

    /// The part of the file name after the last dot, or nothing.
    pub fn extension(&self) -> &str {
        self.file_name().rsplit_once('.').map_or("", |(_, ext)| ext)
    }
}

impl PodArchive {
    /// How many bytes at the start of an archive hold its header and directory, which is
    /// all [`parse_directory`](Self::parse_directory) needs. `header` is the first
    /// [`HEADER_LENGTH`] bytes.
    pub fn directory_end(header: &[u8]) -> Result<usize, PodError> {
        let header = slice(header, 0, HEADER_LENGTH, "archive header")?;
        let count = read_u32(header, 0);
        Ok(HEADER_LENGTH.saturating_add(count.saturating_mul(ENTRY_LENGTH)))
    }

    /// The comment and the directory of an archive, from its first
    /// [`directory_end`](Self::directory_end) bytes, without its files. What the entries
    /// point at is not checked, because it is not there.
    pub fn parse_directory(bytes: &[u8]) -> Result<(String, Vec<Entry>), PodError> {
        let header = slice(bytes, 0, HEADER_LENGTH, "archive header")?;
        let count = read_u32(header, 0);
        let comment = read_string(&header[4..]);

        // Checked before allocating, so a nonsense count can't ask for gigabytes.
        let directory_length = count.saturating_mul(ENTRY_LENGTH);
        let directory = slice(bytes, HEADER_LENGTH, directory_length, "file directory")?;

        let entries = directory
            .as_chunks::<ENTRY_LENGTH>()
            .0
            .iter()
            .map(|record| Entry {
                name: read_string(&record[..NAME_LENGTH]),
                size: read_u32(record, NAME_LENGTH),
                offset: read_u32(record, NAME_LENGTH + 4),
            })
            .collect();
        Ok((comment, entries))
    }

    pub fn parse(bytes: Vec<u8>) -> Result<Self, PodError> {
        let (comment, entries) = Self::parse_directory(&bytes)?;
        for entry in &entries {
            slice(&bytes, entry.offset, entry.size, &entry.name)?;
        }
        Ok(Self {
            comment,
            entries,
            bytes,
        })
    }

    /// The free-text description stored in the header.
    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn data(&self, entry: &Entry) -> &[u8] {
        &self.bytes[entry.offset..entry.offset + entry.size]
    }

    /// Finds a file by name alone, ignoring folders and case. This is how the game's
    /// own files refer to each other: a level asks for `nutrack.raw`, which the archive
    /// holds as `DATA\NUTRACK.RAW`. When several folders hold the name, the first wins:
    /// use [`find_file_in`](Self::find_file_in) when it matters which.
    pub fn find_file(&self, file_name: &str) -> Option<&Entry> {
        let file_name = file_name.trim();
        self.entries
            .iter()
            .find(|entry| entry.file_name().eq_ignore_ascii_case(file_name))
    }

    /// Finds a file by name, preferring the copy in `folder` (ignoring case) and falling
    /// back to any folder. Each kind of file has a folder of its own, and names repeat
    /// across them: a track's heightmap `DATA\MONTECAR.RAW` often has a ground texture
    /// `ART\MONTECAR.RAW` beside it.
    pub fn find_file_in(&self, folder: &str, file_name: &str) -> Option<&Entry> {
        let file_name = file_name.trim();
        let mut any = None;
        for entry in &self.entries {
            if !entry.file_name().eq_ignore_ascii_case(file_name) {
                continue;
            }
            if entry.folder().eq_ignore_ascii_case(folder) {
                return Some(entry);
            }
            any.get_or_insert(entry);
        }
        any
    }
}

fn slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    length: usize,
    what: &str,
) -> Result<&'a [u8], PodError> {
    offset
        .checked_add(length)
        .and_then(|end| bytes.get(offset..end))
        .ok_or_else(|| PodError::Truncated {
            what: what.to_string(),
            offset,
            needed: length,
            available: bytes.len().saturating_sub(offset),
        })
}

fn read_u32(bytes: &[u8], offset: usize) -> usize {
    let raw: [u8; 4] = bytes[offset..offset + 4].try_into().expect("four bytes");
    u32::from_le_bytes(raw) as usize
}

/// A fixed-width field holding a NUL-terminated string.
fn read_string(field: &[u8]) -> String {
    let end = field
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(field.len());
    field[..end].iter().map(|&byte| byte as char).collect()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// Builds an archive the way the format lays it out: header, directory, then data.
    pub(crate) fn build_archive(comment: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = (files.len() as u32).to_le_bytes().to_vec();
        bytes.extend(fixed_width(comment, COMMENT_LENGTH));

        let mut offset = HEADER_LENGTH + files.len() * ENTRY_LENGTH;
        for (name, data) in files {
            bytes.extend(fixed_width(name, NAME_LENGTH));
            bytes.extend((data.len() as u32).to_le_bytes());
            bytes.extend((offset as u32).to_le_bytes());
            offset += data.len();
        }
        for (_, data) in files {
            bytes.extend_from_slice(data);
        }
        bytes
    }

    fn fixed_width(text: &str, width: usize) -> Vec<u8> {
        let mut field = text.as_bytes().to_vec();
        field.resize(width, 0);
        field
    }

    #[test]
    fn reads_comment_entries_and_data() {
        let bytes = build_archive(
            "A test track",
            &[
                ("DATA\\HILLS.RAW", &[1, 2, 3]),
                ("WORLD\\HILLS.SIT", b"hello"),
            ],
        );
        let archive = PodArchive::parse(bytes).unwrap();

        assert_eq!(archive.comment(), "A test track");
        assert_eq!(archive.entries().len(), 2);
        assert_eq!(archive.entries()[0].name, "DATA\\HILLS.RAW");
        assert_eq!(archive.entries()[0].file_name(), "HILLS.RAW");
        assert_eq!(archive.entries()[0].extension(), "RAW");
        assert_eq!(archive.data(&archive.entries()[0]), [1, 2, 3]);
        assert_eq!(archive.data(&archive.entries()[1]), b"hello");
    }

    #[test]
    fn finds_files_by_name_ignoring_folder_and_case() {
        let bytes = build_archive("", &[("DATA\\HILLS.RAW", &[9])]);
        let archive = PodArchive::parse(bytes).unwrap();
        assert!(archive.find_file("hills.raw").is_some());
        assert!(archive.find_file(" Hills.Raw\r").is_some());
        assert!(archive.find_file("hills.clr").is_none());
        assert!(archive.find_file("DATA\\HILLS.RAW").is_none());
    }

    #[test]
    fn prefers_the_named_folder_and_falls_back_to_any() {
        let bytes = build_archive(
            "",
            &[
                ("ART\\HILLS.RAW", &[1]),
                ("DATA\\HILLS.RAW", &[2]),
                ("ART\\HILLS.ACT", &[3]),
            ],
        );
        let archive = PodArchive::parse(bytes).unwrap();
        assert_eq!(archive.entries()[1].folder(), "DATA");
        assert_eq!(
            archive.find_file("hills.raw").unwrap().name,
            "ART\\HILLS.RAW"
        );
        assert_eq!(
            archive.find_file_in("data", "hills.raw").unwrap().name,
            "DATA\\HILLS.RAW"
        );
        assert_eq!(
            archive.find_file_in("ART", "hills.raw").unwrap().name,
            "ART\\HILLS.RAW"
        );
        // Only ART holds the palette, so asking for DATA still finds it.
        assert_eq!(
            archive.find_file_in("DATA", "hills.act").unwrap().name,
            "ART\\HILLS.ACT"
        );
        assert!(archive.find_file_in("DATA", "hills.clr").is_none());
    }

    #[test]
    fn an_empty_archive_is_fine() {
        let archive = PodArchive::parse(build_archive("empty", &[])).unwrap();
        assert!(archive.entries().is_empty());
    }

    #[test]
    fn rejects_truncated_input_without_panicking() {
        let bytes = build_archive("", &[("A.RAW", &[1, 2, 3, 4])]);
        for length in 0..bytes.len() {
            let result = PodArchive::parse(bytes[..length].to_vec());
            assert!(
                matches!(result, Err(PodError::Truncated { .. })),
                "at {length}"
            );
        }
    }

    #[test]
    fn reads_the_directory_from_the_start_of_the_archive_alone() {
        let bytes = build_archive("base", &[("ART\\A.RAW", &[1, 2, 3]), ("B.ACT", &[4])]);
        let end = PodArchive::directory_end(&bytes[..HEADER_LENGTH]).unwrap();
        assert_eq!(end, HEADER_LENGTH + 2 * ENTRY_LENGTH);

        let (comment, entries) = PodArchive::parse_directory(&bytes[..end]).unwrap();
        assert_eq!(comment, "base");
        assert_eq!(entries, PodArchive::parse(bytes).unwrap().entries());
    }

    #[test]
    fn rejects_an_absurd_file_count() {
        let mut bytes = build_archive("", &[]);
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            PodArchive::parse(bytes),
            Err(PodError::Truncated { .. })
        ));
    }
}
