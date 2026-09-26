//! The base game's archives on disk, from the player's own copy of Monster Truck Madness 2,
//! which tracks and trucks borrow textures, models and palettes from.
//!
//! Not a slice: a helper for `track` and `truck`, which may not use each other, and the
//! file-system side of `pod::BaseArchives`, which `pod` cannot be. When it opens, it reads
//! only each archive's header and directory, a few kilobytes, and remembers which archive
//! holds which file. An archive is read in full the first time a track or truck asks for a
//! file in it, and kept for later loads. An archive that no track or truck borrows from is
//! never read.
//!
//! No file name is in two of the base game's archives with different contents (measured:
//! 354 names repeat across its 19 archives, and every copy is the same), so the order the
//! archives are searched in does not matter for them. See `docs/formats/pod.md`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bevy::log::{info, warn};
use bevy::prelude::Resource;

use crate::pod::{ARCHIVE_HEADER_LENGTH, BaseArchives, Entry, PodArchive};

/// Where the base archives are looked for when the command line names no folder: beside
/// `tracks/` and `trucks/`, and git ignores it as it does them. Its subfolders are searched
/// too, so that it can hold links to the `Shared` and `English` folders of an MTM2 CD.
pub const DEFAULT_FOLDER: &str = "base";

/// The base game's archives. Cheap to clone: the clones share what has been read.
#[derive(Resource, Clone, Default)]
pub struct BaseGame(Arc<Shelf>);

#[derive(Default)]
struct Shelf {
    archives: Vec<Mounted>,
    /// For each upper-case bare file name, the archives that hold it, by index into
    /// `archives`, with the folder it is in there.
    by_name: HashMap<String, Vec<(usize, String)>>,
}

struct Mounted {
    path: PathBuf,
    /// The archive's directory, read when the shelf is opened.
    entries: Vec<Entry>,
    /// The whole archive, read the first time a file in it is asked for. `None` if it
    /// could not be read then.
    archive: OnceLock<Option<PodArchive>>,
}

impl BaseGame {
    /// No base archives: tracks and trucks have only the files they carry.
    pub fn none() -> Self {
        Self::default()
    }

    /// The archives (`*.pod`, in any case) in each folder and in its subfolders, one
    /// level down. A folder that is missing is skipped, and an archive whose directory
    /// can't be read is skipped with a warning.
    pub fn open(folders: &[PathBuf]) -> Self {
        let mut paths: Vec<PathBuf> = folders
            .iter()
            .flat_map(|folder| {
                let mut found = archives_in(folder);
                for subfolder in subfolders(folder) {
                    found.extend(archives_in(&subfolder));
                }
                found
            })
            .collect();
        paths.dedup();

        let mut shelf = Shelf::default();
        for path in paths {
            let entries = match read_directory(&path) {
                Ok(entries) => entries,
                Err(error) => {
                    warn!("Base archive {} skipped: {error}", path.display());
                    continue;
                }
            };
            let index = shelf.archives.len();
            for entry in &entries {
                shelf
                    .by_name
                    .entry(entry.file_name().to_ascii_uppercase())
                    .or_default()
                    .push((index, entry.folder().to_string()));
            }
            shelf.archives.push(Mounted {
                path,
                entries,
                archive: OnceLock::new(),
            });
        }
        Self(Arc::new(shelf))
    }

    /// How many base archives were found.
    pub fn archive_count(&self) -> usize {
        self.0.archives.len()
    }

    /// Every base archive and its directory, in the order they are searched.
    pub fn directories(&self) -> impl Iterator<Item = (&Path, &[Entry])> {
        self.0
            .archives
            .iter()
            .map(|mounted| (mounted.path.as_path(), mounted.entries.as_slice()))
    }

    /// One file of a base archive, read on its own: the rest of the archive is not read.
    /// For a list of the base game's tracks and trucks, which needs only their names.
    pub fn read_one(&self, archive: &Path, entry: &Entry) -> Result<Vec<u8>, String> {
        let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", archive.display());
        let mut file = File::open(archive).map_err(|error| describe(&error))?;
        file.seek(SeekFrom::Start(entry.offset as u64))
            .map_err(|error| describe(&error))?;
        let mut bytes = Vec::new();
        file.take(entry.size as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| describe(&error))?;
        if bytes.len() != entry.size {
            return Err(describe(&format!("{} is cut short", entry.name)));
        }
        Ok(bytes)
    }

    /// A base archive in full, read now if it has not been. `None` if it is not one of the
    /// base archives, or cannot be read.
    pub fn archive(&self, path: &Path) -> Option<&PodArchive> {
        self.0
            .archives
            .iter()
            .find(|mounted| mounted.path == path)?
            .read()
    }

    /// The archives read in full so far: those a track or truck has borrowed from.
    pub fn archives_read(&self) -> Vec<&Path> {
        self.0
            .archives
            .iter()
            .filter(|mounted| mounted.archive.get().is_some_and(Option::is_some))
            .map(|mounted| mounted.path.as_path())
            .collect()
    }
}

impl BaseArchives for BaseGame {
    fn holding(&self, folder: &str, file_name: &str) -> Option<&PodArchive> {
        let holders = self.0.by_name.get(&file_name.trim().to_ascii_uppercase())?;
        // Those with the file in the folder asked for first, as inside one archive.
        let in_folder = holders
            .iter()
            .filter(|(_, found_in)| found_in.eq_ignore_ascii_case(folder));
        let elsewhere = holders
            .iter()
            .filter(|(_, found_in)| !found_in.eq_ignore_ascii_case(folder));
        in_folder
            .chain(elsewhere)
            .find_map(|&(index, _)| self.0.archives[index].read())
    }
}

impl Mounted {
    fn read(&self) -> Option<&PodArchive> {
        self.archive
            .get_or_init(|| {
                info!("Reading base archive {}", self.path.display());
                let archive = std::fs::read(&self.path)
                    .map_err(|error| error.to_string())
                    .and_then(|bytes| PodArchive::parse(bytes).map_err(|error| error.to_string()));
                archive
                    .inspect_err(|error| warn!("Base archive {}: {error}", self.path.display()))
                    .ok()
            })
            .as_ref()
    }
}

/// An archive's header and directory, without reading the rest of it.
fn read_directory(path: &Path) -> Result<Vec<Entry>, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = vec![0; ARCHIVE_HEADER_LENGTH];
    file.read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    let end = PodArchive::directory_end(&bytes).map_err(|error| error.to_string())?;
    // `take` stops a nonsense file count from asking for more than the file holds.
    file.take((end - ARCHIVE_HEADER_LENGTH) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let (_, entries) = PodArchive::parse_directory(&bytes).map_err(|error| error.to_string())?;
    Ok(entries)
}

/// The archives directly in a folder, in name order. Links are followed. Also for the
/// front end, which asks the player for a folder with archives in it.
pub fn archives_in(folder: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = entries(folder)
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pod"))
        })
        .collect();
    paths.sort();
    paths
}

fn subfolders(folder: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = entries(folder).filter(|path| path.is_dir()).collect();
    paths.sort();
    paths
}

fn entries(folder: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
}
