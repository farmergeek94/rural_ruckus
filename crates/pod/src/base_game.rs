//! The base game's archives on disk, from the player's own copy of Monster Truck Madness 2,
//! which tracks and trucks borrow textures, models and palettes from.
//!
//! The file-system side of `crate::BaseArchives`, which `pod` cannot be. It is opened on
//! the folder where MTM2 is installed, and loads what that folder's `POD.INI` names, as
//! MTM2 does: the base game's 19 archives, and after them every add-on track and truck
//! the player installed (`archives_listed`). A folder with no `POD.INI`, such as links to
//! a CD's `Shared` and `English` folders, gives every archive in it and in its subfolders.
//! When it opens, it reads only each archive's header and directory, a few kilobytes, and
//! remembers which archive holds which file. An archive is read in full the first time a
//! track or truck asks for a file in it, and kept for later loads. An archive that no
//! track or truck borrows from is never read.
//!
//! No file name is in two of the base game's archives with different contents (measured:
//! 354 names repeat across its 19 archives, and every copy is the same), so the order the
//! archives are searched in does not matter for them. Between an add-on and the base game
//! the first archive in `POD.INI`'s order wins; what MTM2 does is open. See
//! `docs/formats/pod.md`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use log::{info, warn};

use crate::{ARCHIVE_HEADER_LENGTH, BaseArchives, Entry, PodArchive};

/// The base game's archives. Cheap to clone: the clones share what has been read.
#[derive(Clone, Default)]
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

    /// The archives of each game folder, in `POD.INI`'s order (`archives_listed`). A
    /// folder that is missing is skipped, and an archive whose directory can't be read is
    /// skipped with a warning.
    pub fn open(folders: &[PathBuf]) -> Self {
        let mut paths: Vec<PathBuf> = Vec::new();
        for path in folders.iter().flat_map(|folder| archives_listed(folder)) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }

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

/// The name of the file in MTM2's folder that lists the archives it loads.
pub const POD_INI: &str = "POD.INI";

/// The archives a game folder says to load, in the order to search them. With a `POD.INI`
/// in the folder (in any case), those it names, in its order; a name it gives that is
/// not there is skipped with a warning. Without one, every archive in the folder and in
/// its subfolders one level down, in name order. Nothing is read of the archives.
///
/// Also for the front end, which asks the player for the game's folder.
pub fn archives_listed(folder: &Path) -> Vec<PathBuf> {
    let Some(ini) = pod_ini_in(folder) else {
        let mut found = archives_in(folder);
        for subfolder in subfolders(folder) {
            found.extend(archives_in(&subfolder));
        }
        return found;
    };
    let text = match std::fs::read(&ini) {
        // MTM2's files are ANSI; the names in them are ASCII.
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            warn!("{} cannot be read: {error}", ini.display());
            return Vec::new();
        }
    };
    parse_pod_ini(&text)
        .into_iter()
        .filter_map(|name| {
            let found = resolve_archive(folder, &name);
            if found.is_none() {
                warn!(
                    "{} names {name}, which is not in {}",
                    ini.display(),
                    folder.display()
                );
            }
            found
        })
        .collect()
}

/// The folder's `POD.INI`, in whatever case it is written in.
pub fn pod_ini_in(folder: &Path) -> Option<PathBuf> {
    entries(folder).filter(|path| path.is_file()).find(|path| {
        path.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(POD_INI))
    })
}

/// The archive names a `POD.INI` lists, in its order. The first line is how many there
/// are (reference: MTM2's `README.TXT`, `docs/formats/pod.md`); the lines after it are
/// each one archive, a bare name or a path. The count is not trusted: every line that
/// is not blank is a name, so that a list the player edited by hand loads in full. A
/// first line that is not a number is taken as a name too.
pub fn parse_pod_ini(text: &str) -> Vec<String> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let mut names: Vec<String> = Vec::new();
    if let Some(first) = lines.next()
        && first.parse::<usize>().is_err()
    {
        names.push(first.to_string());
    }
    names.extend(lines.map(str::to_string));
    names
}

/// Where an archive that `POD.INI` names is: a bare name is a file in the folder; a path
/// (MTM2 wrote them with backslashes) is from the folder, or absolute. Case does not
/// matter in any part, as it did not on the file system MTM2 ran on.
fn resolve_archive(folder: &Path, name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    let path = Path::new(&name);
    let mut at = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        folder.to_path_buf()
    };
    for component in path.components() {
        let std::path::Component::Normal(wanted) = component else {
            continue;
        };
        at = entries(&at).find(|found| {
            found
                .file_name()
                .is_some_and(|found| found.eq_ignore_ascii_case(wanted))
        })?;
    }
    at.is_file().then_some(at)
}

/// The archives directly in a folder, in name order. Links are followed.
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

fn entries(folder: &Path) -> impl Iterator<Item = PathBuf> + use<> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_ini_lists_names_after_the_count() {
        let names = parse_pod_ini("19\r\nstartup.pod\r\nmusic.pod\r\n\r\nAddOn\\MyTrack.POD\r\n");
        assert_eq!(names, ["startup.pod", "music.pod", "AddOn\\MyTrack.POD"]);
        // A count that is wrong, or missing, loses nothing.
        assert_eq!(parse_pod_ini("1\na.pod\nb.pod"), ["a.pod", "b.pod"]);
        assert_eq!(parse_pod_ini("a.pod\nb.pod"), ["a.pod", "b.pod"]);
        assert!(parse_pod_ini("").is_empty());
        assert!(parse_pod_ini("0\n").is_empty());
    }

    /// A made-up game folder, gone again when it is dropped.
    struct GameFolder(PathBuf);

    impl GameFolder {
        fn new(test: &str) -> Self {
            let root = std::env::temp_dir().join(format!("pod-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("addons")).unwrap();
            Self(root)
        }

        fn put(&self, name: &str) {
            std::fs::write(self.0.join(name), b"").unwrap();
        }
    }

    impl Drop for GameFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_game_folder_gives_the_archives_its_pod_ini_names() {
        let game = GameFolder::new("ini");
        for name in [
            "STARTUP.POD",
            "TRUCK2.POD",
            "LOOSE.POD",
            "addons/mytrack.pod",
        ] {
            game.put(name);
        }
        // Lower-case names, as MTM2 writes them; a path with a backslash, as the player
        // adds one; and one that is not there.
        game.put("pod.ini");
        std::fs::write(
            game.0.join("pod.ini"),
            "3\r\ntruck2.pod\r\nstartup.pod\r\naddons\\MyTrack.pod\r\ngone.pod\r\n",
        )
        .unwrap();
        assert_eq!(
            archives_listed(&game.0),
            [
                game.0.join("TRUCK2.POD"),
                game.0.join("STARTUP.POD"),
                game.0.join("addons/mytrack.pod"),
            ]
        );
        assert_eq!(pod_ini_in(&game.0), Some(game.0.join("pod.ini")));
    }

    #[test]
    fn a_folder_without_pod_ini_gives_every_archive_in_and_under_it() {
        let game = GameFolder::new("loose");
        for name in ["B.POD", "a.pod", "addons/mytrack.pod", "notes.txt"] {
            game.put(name);
        }
        assert_eq!(pod_ini_in(&game.0), None);
        assert_eq!(
            archives_listed(&game.0),
            [
                game.0.join("B.POD"),
                game.0.join("a.pod"),
                game.0.join("addons/mytrack.pod"),
            ]
        );
    }
}
