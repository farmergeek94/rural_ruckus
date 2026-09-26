//! The base game's archives, which tracks and trucks borrow files from. A track's archive
//! names textures, models and palettes that it does not carry, and the game finds them in
//! its own archives. See `docs/formats/pod.md`.

use super::{Entry, PodArchive};

/// Where the files that an archive names but does not carry are looked for.
///
/// Implemented here for a plain list of archives. The game implements it over archives that
/// it reads from disk only when a file in them is asked for, which `pod` cannot do itself.
pub trait BaseArchives {
    /// The archive that holds a file by this bare name, preferring one that holds it in
    /// `folder`, as [`PodArchive::find_file_in`] does inside one archive.
    fn holding(&self, folder: &str, file_name: &str) -> Option<&PodArchive>;
}

/// No base archives: a track or truck has only the files it carries.
pub struct NoBase;

impl BaseArchives for NoBase {
    fn holding(&self, _folder: &str, _file_name: &str) -> Option<&PodArchive> {
        None
    }
}

/// The first archive that holds the file in `folder`, else the first that holds it at all.
impl BaseArchives for Vec<PodArchive> {
    fn holding(&self, folder: &str, file_name: &str) -> Option<&PodArchive> {
        let mut any = None;
        for archive in self {
            let Some(entry) = archive.find_file_in(folder, file_name) else {
                continue;
            };
            if entry.folder().eq_ignore_ascii_case(folder) {
                return Some(archive);
            }
            any.get_or_insert(archive);
        }
        any
    }
}

/// A track's or truck's own archive, and the base archives behind it. A file is taken
/// from the own archive if it is there, in any folder, and only otherwise from the base.
#[derive(Clone, Copy)]
pub(super) struct Files<'a> {
    own: &'a PodArchive,
    base: &'a dyn BaseArchives,
}

impl<'a> Files<'a> {
    pub(super) fn new(own: &'a PodArchive, base: &'a dyn BaseArchives) -> Self {
        Self { own, base }
    }

    /// The archive a file is in, and its entry there.
    pub(super) fn find_in(
        &self,
        folder: &str,
        file_name: &str,
    ) -> Option<(&'a PodArchive, &'a Entry)> {
        if let Some(entry) = self.own.find_file_in(folder, file_name) {
            return Some((self.own, entry));
        }
        let archive = self.base.holding(folder, file_name)?;
        Some((archive, archive.find_file_in(folder, file_name)?))
    }

    /// A file that goes with another, such as a texture's own palette: from the archive
    /// the other came from, and only from there. A palette in the base game does not colour
    /// a texture that a track carries itself. Where the other is in no archive, the own
    /// archive is searched.
    pub(super) fn data_beside(
        &self,
        other_folder: &str,
        other: &str,
        file_name: &str,
    ) -> Option<&'a [u8]> {
        let archive = self
            .find_in(other_folder, other)
            .map_or(self.own, |(archive, _)| archive);
        let entry = archive.find_file_in(other_folder, file_name)?;
        Some(archive.data(entry))
    }

    /// The bytes of a file, as [`find_in`](Self::find_in) finds it.
    pub(super) fn data_in(&self, folder: &str, file_name: &str) -> Option<&'a [u8]> {
        let (archive, entry) = self.find_in(folder, file_name)?;
        Some(archive.data(entry))
    }
}

#[cfg(test)]
mod tests {
    use super::super::archive::tests::build_archive;
    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> PodArchive {
        PodArchive::parse(build_archive("", files)).unwrap()
    }

    #[test]
    fn the_own_archive_comes_before_the_base() {
        let own = archive(&[("ART\\A.RAW", &[1])]);
        let base = vec![archive(&[("ART\\A.RAW", &[2]), ("ART\\B.RAW", &[3])])];
        let files = Files::new(&own, &base);
        assert_eq!(files.data_in("ART", "a.raw"), Some(&[1][..]));
        assert_eq!(files.data_in("ART", "b.raw"), Some(&[3][..]));
        assert_eq!(files.data_in("ART", "c.raw"), None);
    }

    #[test]
    fn a_base_archive_with_the_file_in_its_folder_comes_first() {
        let base = vec![
            archive(&[("DATA\\A.RAW", &[1])]),
            archive(&[("ART\\A.RAW", &[2])]),
        ];
        let own = archive(&[]);
        let files = Files::new(&own, &base);
        assert_eq!(files.data_in("ART", "A.RAW"), Some(&[2][..]));
        assert_eq!(files.data_in("DATA", "A.RAW"), Some(&[1][..]));
        assert_eq!(files.data_in("MODELS", "A.RAW"), Some(&[1][..]));
    }

    #[test]
    fn a_palette_comes_only_from_its_textures_archive() {
        let own = archive(&[("ART\\OWN.RAW", &[1])]);
        let base = vec![archive(&[
            ("ART\\OWN.ACT", &[2]),
            ("ART\\BORROWED.RAW", &[3]),
            ("ART\\BORROWED.ACT", &[4]),
        ])];
        let files = Files::new(&own, &base);
        assert_eq!(files.data_beside("ART", "OWN.RAW", "OWN.ACT"), None);
        assert_eq!(
            files.data_beside("ART", "BORROWED.RAW", "BORROWED.ACT"),
            Some(&[4][..])
        );
    }

    #[test]
    fn no_base_finds_only_the_own_files() {
        let own = archive(&[("ART\\A.RAW", &[1])]);
        let files = Files::new(&own, &NoBase);
        assert_eq!(files.data_in("ART", "A.RAW"), Some(&[1][..]));
        assert_eq!(files.data_in("ART", "B.RAW"), None);
    }
}
