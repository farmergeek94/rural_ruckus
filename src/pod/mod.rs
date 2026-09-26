//! Reads Monster Truck Madness 2 (Terminal Reality) POD archives and the track and truck
//! files inside them.
//!
//! Everything here describes the files as they are: positions stay in feet, heights in
//! raw steps, angles as written, in MTM2's own axes. Converting to another engine's
//! conventions is the caller's job. There is no file system access and nothing panics
//! on bad input.
//!
//! The formats are documented in `docs/formats/` at the root of the repository, along
//! with where each fact came from.

use std::collections::BTreeMap;

mod archive;
mod base;
mod cockpit;
mod error;
mod ground_boxes;
mod hd_texture;
mod heightmap;
mod level;
mod model;
mod situation;
mod sky;
mod texture_map;
mod texture_types;
mod textures;
mod truck_file;

pub use archive::{Entry, HEADER_LENGTH as ARCHIVE_HEADER_LENGTH, PodArchive};
use base::Files;
pub use base::{BaseArchives, NoBase};
pub use cockpit::{CockpitLayout, Gauge, Mirror, SCREENS, SEE_THROUGH_INDEX};
pub use error::PodError;
pub use ground_boxes::{GroundBox, GroundBoxes, face};
pub use hd_texture::{HdFormat, HdTexture};
pub use heightmap::{FEET_PER_CELL, FEET_PER_HEIGHT_STEP, Heightmap};
pub use level::Level;
pub use model::{
    Corner, Face, KeyframeAnimation, Material, Model, TextureCycle, face_type, material_flag,
};
pub use situation::{BoxShape, CourseSegment, Situation, SituationBox, Vehicle, box_type};
pub use sky::{CLOUDY_SKY, DUSK_SKY, NIGHT_SKY, first_slot, sky_rgba};
pub use texture_map::{TextureCell, TextureMap};
pub use texture_types::{TextureType, TextureTypes};
pub use textures::{Palette, Picture, Texture, parse_texture_list};
pub use truck_file::{Side, Tires, TruckFile, TruckLight, Wheel};

/// The folder each kind of file lives in. Files name each other by bare file name, and
/// the same name turns up in more than one folder, so each lookup says where to look
/// first. See `docs/formats/pod.md`.
pub mod folder {
    /// Textures (.RAW) and palettes (.ACT).
    pub const ART: &str = "ART";
    /// The heightmap (.RAW), texture map (.CLR), texture list (.TEX) and the rest of a
    /// track's terrain.
    pub const DATA: &str = "DATA";
    /// Level files (.LVL).
    pub const LEVELS: &str = "LEVELS";
    /// Models (.BIN).
    pub const MODELS: &str = "MODELS";
    /// Truck files (.TRK).
    pub const TRUCK: &str = "TRUCK";
    /// Situations (.SIT, or .SI2).
    pub const WORLD: &str = "WORLD";
}

/// A track's files, found and parsed by following the references between them: the
/// situation (.SIT) names the level (.LVL), which names the rest.
#[derive(Clone, Debug)]
pub struct Track {
    pub situation: Situation,
    pub level: Level,
    pub heightmap: Heightmap,
    /// Solid blocks on the terrain's grid: bridges, tunnel roofs, walls. `None` if the
    /// archive doesn't have the files, or they can't be read.
    pub ground_boxes: Option<GroundBoxes>,
    /// Which texture covers each terrain cell. Missing from some archives.
    pub texture_map: Option<TextureMap>,
    /// The file names of the ground textures, in the order the texture map's indices
    /// refer to them.
    pub ground_texture_names: Vec<String>,
    /// What kind of surface each texture is. `None` if the archive doesn't have the file,
    /// or it can't be read.
    pub texture_types: Option<TextureTypes>,
    /// The ground textures, in the order the texture map's indices refer to them. A
    /// track that uses the base game's textures doesn't carry them, and those are `None`.
    pub ground_textures: Vec<Option<Texture>>,
    /// The PNG that stands in for each ground texture, in the same order, where the
    /// archive has one (MTM2 Community Patch 3). Where there is one, the game uses it in
    /// place of the .RAW, and an archive may have no .RAW for it at all.
    pub ground_hd_textures: Vec<Option<HdTexture>>,
    /// The palette that colours the track's textures. `None` if it isn't in the archive.
    pub palette: Option<Palette>,
    /// The models that the situation's boxes name, by upper-case file name. Models that
    /// aren't in the archive, which belong to the base game, are simply absent. A box can
    /// name an animation control file: that name gives its first frame here, and its frames
    /// in `animated_models`.
    pub models: BTreeMap<String, Model>,
    /// The animation control files that the situation's boxes name, by upper-case file
    /// name, each with every one of its frames. One whose frames can't all be read, or
    /// don't all have as many vertices as the first, is absent, and its first frame is
    /// drawn still.
    pub animated_models: BTreeMap<String, AnimatedModel>,
    /// The models that the situation's Backdrop section names, drawn round the horizon, by
    /// upper-case file name. Those not in the archive are absent, as for `models`.
    pub backdrop_models: BTreeMap<String, Model>,
    /// The textures all those models use, by upper-case file name, likewise.
    pub model_textures: BTreeMap<String, Texture>,
    /// The PNGs that stand in for those textures, by the upper-case file name the models
    /// use, as for `ground_hd_textures`.
    pub model_hd_textures: BTreeMap<String, HdTexture>,
    /// A texture's own palette, where the archive has one beside it with the same name
    /// (`CROKDROP.ACT` beside `CROKDROP.RAW`), by the texture's upper-case file name: for
    /// the ground textures and all the models' textures. It colours that texture in place
    /// of the track's palette. See `docs/formats/textures.md`.
    pub own_palettes: BTreeMap<String, Palette>,
    /// The skies: the track's own, which its level names, and the base game's weather
    /// skies. Each is `None` where neither the archive nor the base game's has it.
    pub skies: Skies,
}

/// A sky's picture and its palette. Colour it with `sky_rgba`.
#[derive(Clone, Debug, PartialEq)]
pub struct Sky {
    pub texture: Texture,
    pub palette: Palette,
}

/// The track's own sky, and the base game's cloudy, dusk and night skies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skies {
    pub own: Option<Sky>,
    pub cloudy: Option<Sky>,
    pub dusk: Option<Sky>,
    pub night: Option<Sky>,
}

impl Track {
    /// The name of the first track in the archive, without reading the rest of it. `None`
    /// if the archive holds no track.
    pub fn name_in(archive: &PodArchive) -> Option<String> {
        let entry = situation_entry(archive)?;
        Some(Situation::parse_name(&text(archive.data(entry))))
    }

    /// Loads the first track in the archive, with only the files it carries.
    pub fn from_archive(archive: &PodArchive) -> Result<Self, PodError> {
        Self::from_archives(archive, &NoBase)
    }

    /// Loads the first track in the archive. The textures, palettes and models it names
    /// but does not carry are taken from `base`. Its own files, from the track file to the
    /// texture list, must be in its archive.
    pub fn from_archives(archive: &PodArchive, base: &dyn BaseArchives) -> Result<Self, PodError> {
        let situation_entry = situation_entry(archive)
            .ok_or_else(|| PodError::MissingFile("a .SIT or .SI2 track file".into()))?;
        Self::from_entry(archive, situation_entry, base)
    }

    /// The track files in an archive's directory, of which the base game's archives can
    /// hold more than one (`JUNK.POD` holds The Graveyard and Scrapyard Run): every .SIT, or
    /// every .SI2 where there is no .SIT, in directory order.
    pub fn files_in(entries: &[Entry]) -> Vec<&Entry> {
        let with = |extension: &str| -> Vec<&Entry> {
            entries
                .iter()
                .filter(|entry| entry.extension().eq_ignore_ascii_case(extension))
                .collect()
        };
        let sit = with("sit");
        if sit.is_empty() { with("si2") } else { sit }
    }

    /// The name a track file gives its track, from the file's bytes. Empty for a track
    /// that gives none.
    pub fn name_from(bytes: &[u8]) -> String {
        Situation::parse_name(&text(bytes))
    }

    /// Loads the track whose track file is `file_name` (such as `WORLD\JUNK.SIT`, or just
    /// `JUNK.SIT`), as [`from_archives`](Self::from_archives) loads the first.
    pub fn from_file(
        archive: &PodArchive,
        file_name: &str,
        base: &dyn BaseArchives,
    ) -> Result<Self, PodError> {
        let bare = file_name.rsplit(['\\', '/']).next().unwrap_or(file_name);
        let situation_entry = find(archive, folder::WORLD, bare)?;
        Self::from_entry(archive, situation_entry, base)
    }

    fn from_entry(
        archive: &PodArchive,
        situation_entry: &Entry,
        base: &dyn BaseArchives,
    ) -> Result<Self, PodError> {
        let files = Files::new(archive, base);
        let situation = Situation::parse(&text(archive.data(situation_entry)))
            .map_err(|error| error.in_file(&situation_entry.name))?;

        let level_entry = find(archive, folder::LEVELS, &situation.level_file)?;
        let level = Level::parse(&text(archive.data(level_entry)))
            .map_err(|error| error.in_file(&level_entry.name))?;

        let heightmap_entry = find(archive, folder::DATA, &level.heightmap_file)?;
        let heightmap = Heightmap::parse(archive.data(heightmap_entry))
            .map_err(|error| error.in_file(&heightmap_entry.name))?;

        let texture_map = archive
            .find_file_in(folder::DATA, &level.texture_map_file)
            .and_then(|entry| TextureMap::parse(archive.data(entry), heightmap.size()).ok());

        let ground_boxes = read_ground_boxes(archive, &level.heightmap_file, heightmap.size());
        let texture_types = read_texture_types(archive, &level.heightmap_file);

        let ground_texture_names =
            match archive.find_file_in(folder::DATA, &level.texture_list_file) {
                Some(entry) => parse_texture_list(&text(archive.data(entry)))
                    .map_err(|error| error.in_file(&entry.name))?,
                None => Vec::new(),
            };
        let ground_textures = ground_texture_names
            .iter()
            .map(|name| Texture::parse(files.data_in(folder::ART, name)?).ok())
            .collect();
        let has_hd_textures = hd_texture::any_in(archive);
        let ground_hd_textures = ground_texture_names
            .iter()
            .map(|name| {
                has_hd_textures
                    .then(|| hd_texture::find(archive, name))
                    .flatten()
            })
            .collect();
        let palette = files
            .data_in(folder::ART, &level.palette_file)
            .and_then(|bytes| Palette::parse(bytes).ok());

        let mut models = BTreeMap::new();
        let mut animated_models = BTreeMap::new();
        for situation_box in &situation.boxes {
            let BoxShape::Model(name) = &situation_box.shape else {
                continue;
            };
            let key = name.to_ascii_uppercase();
            if models.contains_key(&key) {
                continue;
            }
            // A model that can't be read costs the track one kind of scenery, not the race.
            if let Some((model, animated)) = read_model(&files, name) {
                if let Some(animated) = animated {
                    animated_models.insert(key.clone(), animated);
                }
                models.insert(key, model);
            }
        }

        let mut backdrop_models = BTreeMap::new();
        for name in &situation.backdrops {
            let key = name.to_ascii_uppercase();
            if let Some(model) = files
                .data_in(folder::MODELS, name)
                .and_then(|bytes| Model::parse(bytes).ok())
            {
                backdrop_models.insert(key, model);
            }
        }

        let mut own_palettes = BTreeMap::new();
        let model_texture_names = models
            .values()
            .chain(backdrop_models.values())
            .flat_map(Model::textures);
        for name in ground_texture_names
            .iter()
            .map(String::as_str)
            .chain(model_texture_names)
        {
            let key = name.to_ascii_uppercase();
            if own_palettes.contains_key(&key) {
                continue;
            }
            let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
            if let Some(palette) = files
                .data_beside(folder::ART, name, &format!("{stem}.ACT"))
                .and_then(|bytes| Palette::parse(bytes).ok())
            {
                own_palettes.insert(key, palette);
            }
        }

        let mut model_textures = BTreeMap::new();
        let mut model_hd_textures = BTreeMap::new();
        let all_models = models.values().chain(backdrop_models.values());
        for name in all_models.flat_map(Model::textures) {
            let key = name.to_ascii_uppercase();
            if model_textures.contains_key(&key) || model_hd_textures.contains_key(&key) {
                continue;
            }
            if let Some(texture) = files
                .data_in(folder::ART, name)
                .and_then(|bytes| Texture::parse(bytes).ok())
            {
                model_textures.insert(key.clone(), texture);
            }
            if has_hd_textures && let Some(texture) = hd_texture::find(archive, name) {
                model_hd_textures.insert(key, texture);
            }
        }

        let skies = Skies {
            own: level
                .sky_file
                .as_deref()
                .zip(level.sky_palette_file.as_deref())
                .and_then(|(name, palette)| read_sky(&files, name, palette)),
            cloudy: read_sky(&files, CLOUDY_SKY, &palette_beside(CLOUDY_SKY)),
            dusk: read_sky(&files, DUSK_SKY, &palette_beside(DUSK_SKY)),
            night: read_sky(&files, NIGHT_SKY, &palette_beside(NIGHT_SKY)),
        };

        Ok(Self {
            situation,
            level,
            heightmap,
            ground_boxes,
            texture_map,
            ground_texture_names,
            texture_types,
            ground_textures,
            ground_hd_textures,
            palette,
            models,
            animated_models,
            backdrop_models,
            model_textures,
            model_hd_textures,
            own_palettes,
            skies,
        })
    }
}

/// The sky whose picture is `name`, with the palette `palette_name`, from the archive or
/// the base game's. `None` if either is missing or can't be read.
fn read_sky(files: &Files, name: &str, palette_name: &str) -> Option<Sky> {
    Some(Sky {
        texture: Texture::parse(files.data_in(folder::ART, name)?).ok()?,
        palette: Palette::parse(files.data_in(folder::ART, palette_name)?).ok()?,
    })
}

/// The palette beside a picture: the same name, ending `.ACT`.
fn palette_beside(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    format!("{stem}.ACT")
}

/// A truck's files, found and parsed by following the names in its truck file (.TRK).
///
/// Trucks borrow parts from the base game's archives, tires and axles above all, so any
/// part may be missing: that is `None`, and not an error.
#[derive(Clone, Debug)]
pub struct Truck {
    pub file: TruckFile,
    pub body: Option<Model>,
    /// The finest tire model in the archive, for each side.
    pub left_tire: Option<Model>,
    pub right_tire: Option<Model>,
    /// The tire models an MTM2.1 truck made for one wheel alone, which take the place of
    /// the one for its side: front left, front right, rear left, rear right.
    pub wheel_tires: [Option<Model>; 4],
    pub axle: Option<Model>,
    /// The textures those models, the axle bars and the shocks use, by upper-case file name.
    pub textures: BTreeMap<String, TruckTexture>,
    /// The PNGs or TGAs that stand in for those textures (MTM2 Community Patch 3), by the
    /// same names. Where there is one, the game uses it in place of the .RAW.
    pub hd_textures: BTreeMap<String, HdTexture>,
    /// The normal maps of those textures (Community Patch 3), by the same names.
    pub normal_maps: BTreeMap<String, HdTexture>,
    /// The dashboard that the truck file's `Instrument Cluster` names, which is in the base
    /// game's `COCKPIT.POD`. `None` where it can't be found or read.
    pub cockpit: Option<Cockpit>,
}

/// A dashboard: its layout file and the pictures that it names, each with its own palette.
/// See `docs/formats/cockpit.md`.
#[derive(Clone, Debug)]
pub struct Cockpit {
    /// The layout file's name as found, such as `POWERBIG.480`.
    pub file: String,
    /// The screen the layout is made for, in pixels across and down.
    pub screen: [u32; 2],
    pub layout: CockpitLayout,
    /// The layout's backgrounds, in its order: ahead, left, right and back. `None` where
    /// the picture or its palette is missing, or the picture is not the screen's size.
    pub backgrounds: Vec<Option<CockpitPicture>>,
    /// The steering wheel's pictures, as big as the layout's steering wheel, each with the
    /// number in its name, positive to the left: `PW480L05` is 5, `PW480R05` is -5 and
    /// `PW480C00` 0. In order of that number.
    pub steering_wheel: Vec<(i32, CockpitPicture)>,
}

/// A picture of a dashboard and the palette beside it, which is its own.
#[derive(Clone, Debug, PartialEq)]
pub struct CockpitPicture {
    pub picture: Picture,
    pub palette: Palette,
}

/// How the steering wheel's pictures are named after the layout's base name: `C00`, then
/// `L` or `R` and a number in steps of 5. **Measured** in the base game's `COCKPIT.POD`,
/// which has 5 to 35 each way. Numbers up to this are looked for.
const MOST_WHEEL_TURN: i32 = 90;
const WHEEL_TURN_STEP: usize = 5;

/// The palette of a truck texture that has none of its own beside it: in the base game's
/// `STARTUP.POD`. **Reference**, see `docs/formats/truck.md`.
const FALLBACK_TRUCK_PALETTE: &str = "METALCR2.ACT";

/// Unlike a track's, each of a truck's textures is coloured by a palette of its own: the
/// .ACT file of the same name.
#[derive(Clone, Debug)]
pub struct TruckTexture {
    pub texture: Texture,
    pub palette: Palette,
}

impl Truck {
    /// Whether the archive holds a truck.
    pub fn is_in(archive: &PodArchive) -> bool {
        truck_entry(archive).is_some()
    }

    /// The name of the first truck in the archive, without reading its parts. `None` if
    /// the archive holds no truck.
    pub fn name_in(archive: &PodArchive) -> Option<Result<String, PodError>> {
        let entry = truck_entry(archive)?;
        Some(
            TruckFile::parse_name(&text(archive.data(entry)))
                .map_err(|error| error.in_file(&entry.name)),
        )
    }

    /// Loads the first truck in the archive, with only the parts it carries.
    pub fn from_archive(archive: &PodArchive) -> Result<Self, PodError> {
        Self::from_archives(archive, &NoBase)
    }

    /// Loads the first truck in the archive. The parts, textures and palettes it names but
    /// does not carry are taken from `base`.
    pub fn from_archives(archive: &PodArchive, base: &dyn BaseArchives) -> Result<Self, PodError> {
        let entry = truck_entry(archive)
            .ok_or_else(|| PodError::MissingFile("a .TRK truck file".into()))?;
        Self::from_entry(archive, entry, base)
    }

    /// The truck files in an archive's directory, in directory order. The base game's
    /// `TRUCK2.POD` holds all 20 of its trucks.
    pub fn files_in(entries: &[Entry]) -> Vec<&Entry> {
        entries
            .iter()
            .filter(|entry| entry.extension().eq_ignore_ascii_case("trk"))
            .collect()
    }

    /// The name a truck file gives its truck, from the file's bytes.
    pub fn name_from(bytes: &[u8]) -> Result<String, PodError> {
        TruckFile::parse_name(&text(bytes))
    }

    /// Loads the truck whose truck file is `file_name` (such as `TRUCK\BIGFOOT.TRK`, or just
    /// `BIGFOOT.TRK`), as [`from_archives`](Self::from_archives) loads the first.
    pub fn from_file(
        archive: &PodArchive,
        file_name: &str,
        base: &dyn BaseArchives,
    ) -> Result<Self, PodError> {
        let bare = file_name.rsplit(['\\', '/']).next().unwrap_or(file_name);
        let entry = find(archive, folder::TRUCK, bare)?;
        Self::from_entry(archive, entry, base)
    }

    fn from_entry(
        archive: &PodArchive,
        entry: &Entry,
        base: &dyn BaseArchives,
    ) -> Result<Self, PodError> {
        let files = Files::new(archive, base);
        let file = TruckFile::parse(&text(archive.data(entry)))
            .map_err(|error| error.in_file(&entry.name))?;

        // A part that can't be read costs the truck that part, not the race.
        let model = |name: &str| {
            files
                .data_in(folder::MODELS, name)
                .and_then(|bytes| Model::parse(bytes).ok())
        };
        let tire = |side| {
            file.tire_model_names(side)
                .iter()
                .find_map(|name| model(name))
        };
        let body = model(&file.body_model);
        let (left_tire, right_tire) = (tire(Side::Left), tire(Side::Right));
        let wheel_tires = [
            Wheel::FrontLeft,
            Wheel::FrontRight,
            Wheel::RearLeft,
            Wheel::RearRight,
        ]
        .map(|wheel| model(&file.wheel_tire_model_name(wheel)?));
        let axle = model(&file.axle_model);

        let has_hd_textures = hd_texture::any_in(archive);
        let mut textures = BTreeMap::new();
        let mut hd_textures = BTreeMap::new();
        let mut normal_maps = BTreeMap::new();
        let models = [&body, &left_tire, &right_tire, &axle]
            .into_iter()
            .chain(&wheel_tires);
        let names = models
            .flatten()
            .flat_map(Model::textures)
            .chain(file.bar_texture.as_deref())
            .chain(file.shock_texture.as_deref())
            // The lamps' pictures, and their beams', whose colour is the light's.
            .chain(file.lights.iter().map(|light| light.bitmap.as_str()))
            .chain(file.lights.iter().map(|light| light.cone_texture.as_str()));
        for name in names {
            let key = name.to_ascii_uppercase();
            if textures.contains_key(&key) || hd_textures.contains_key(&key) {
                continue;
            }
            if has_hd_textures {
                if let Some(texture) = hd_texture::find(archive, name) {
                    hd_textures.insert(key.clone(), texture);
                }
                if let Some(normal_map) = hd_texture::find_normal_map(archive, name) {
                    normal_maps.insert(key.clone(), normal_map);
                }
            }
            let stem = key.rsplit_once('.').map_or(key.as_str(), |(stem, _)| stem);
            let texture = files
                .data_in(folder::ART, name)
                .and_then(|bytes| Texture::parse(bytes).ok());
            // A palette beside it that can't be read is no palette: 08NaturalHigh and
            // Slingshot carry a `BLACK.ACT` that isn't one.
            let palette = files
                .data_beside(folder::ART, name, &format!("{stem}.ACT"))
                .and_then(|bytes| Palette::parse(bytes).ok())
                .or_else(|| {
                    Palette::parse(files.data_in(folder::ART, FALLBACK_TRUCK_PALETTE)?).ok()
                });
            if let (Some(texture), Some(palette)) = (texture, palette) {
                textures.insert(key, TruckTexture { texture, palette });
            }
        }

        let cockpit = file
            .instrument_cluster
            .as_deref()
            .and_then(|name| read_cockpit(&files, name));

        Ok(Self {
            file,
            body,
            left_tire,
            right_tire,
            wheel_tires,
            axle,
            textures,
            hd_textures,
            normal_maps,
            cockpit,
        })
    }
}

/// The dashboard whose layout is `name` with the extension of the largest screen there is a
/// layout for. A layout that can't be read gives way to the next.
fn read_cockpit(files: &Files, name: &str) -> Option<Cockpit> {
    SCREENS.iter().find_map(|&(extension, screen)| {
        let file = format!("{name}.{extension}");
        let layout = CockpitLayout::parse(&text(files.data_in(folder::DATA, &file)?)).ok()?;
        let [width, height] = screen.map(|pixels| pixels as usize);
        let backgrounds = layout
            .backgrounds
            .iter()
            .map(|name| cockpit_picture(files, name, width, height))
            .collect();
        let steering_wheel = match (&layout.steering_wheel_base, layout.steering_wheel) {
            (Some(base), Some([_, _, width, height])) => {
                steering_wheel(files, base, width as usize, height as usize)
            }
            _ => Vec::new(),
        };
        Some(Cockpit {
            file: file.to_ascii_uppercase(),
            screen,
            layout,
            backgrounds,
            steering_wheel,
        })
    })
}

fn steering_wheel(
    files: &Files,
    base: &str,
    width: usize,
    height: usize,
) -> Vec<(i32, CockpitPicture)> {
    (-MOST_WHEEL_TURN..=MOST_WHEEL_TURN)
        .step_by(WHEEL_TURN_STEP)
        .filter_map(|turn| {
            let side = match turn.signum() {
                1 => 'L',
                -1 => 'R',
                _ => 'C',
            };
            let name = format!("{base}{side}{:02}.RAW", turn.abs());
            Some((turn, cockpit_picture(files, &name, width, height)?))
        })
        .collect()
}

/// A picture of the size its layout gives, with the palette beside it.
fn cockpit_picture(
    files: &Files,
    name: &str,
    width: usize,
    height: usize,
) -> Option<CockpitPicture> {
    let picture = Picture::parse(files.data_in(folder::ART, name)?, width, height).ok()?;
    let palette =
        Palette::parse(files.data_beside(folder::ART, name, &palette_beside(name))?).ok()?;
    Some(CockpitPicture { picture, palette })
}

/// A model that moves: the frames an animation control file names, which are models with
/// the same faces and vertices in other places. See `docs/formats/model.md`.
#[derive(Clone, Debug)]
pub struct AnimatedModel {
    pub animation: KeyframeAnimation,
    /// Where each frame puts the vertices, in feet, in the same order and axes as
    /// `Model::vertices`: one list for each of `animation.frames`, all as long as the
    /// first, which is the model's own.
    pub frames: Vec<Vec<[f32; 3]>>,
}

/// The model a box names: a model file, or the first frame of an animation control file
/// with where all its frames put the vertices. `None` if neither can be read.
///
/// In every file seen, all the frames of one animation have as many vertices as the first.
/// A frame that doesn't, or can't be read, leaves the model still. Each frame is put in
/// feet by its own magnification, as any model is.
fn read_model(files: &Files, name: &str) -> Option<(Model, Option<AnimatedModel>)> {
    let bytes = files.data_in(folder::MODELS, name)?;
    if !KeyframeAnimation::is_in(bytes) {
        return Some((Model::parse(bytes).ok()?, None));
    }
    let animation = KeyframeAnimation::parse(bytes).ok()?;
    let frame = |name: &String| Model::parse(files.data_in(folder::MODELS, name)?).ok();
    let base = frame(&animation.frames[0])?;
    let frames = animation
        .frames
        .iter()
        .map(|name| {
            frame(name)
                .map(|model| model.vertices)
                .filter(|vertices| vertices.len() == base.vertices.len())
        })
        .collect::<Option<Vec<_>>>();
    let animated = frames.map(|frames| AnimatedModel { animation, frames });
    Some((base, animated))
}

/// The level file doesn't name the ground box files. They are the heightmap's name with
/// other extensions, in every archive seen (see `docs/formats/ground_boxes.md`). Boxes that
/// can't be read cost the track its boxes, not the race.
fn read_ground_boxes(
    archive: &PodArchive,
    heightmap_file: &str,
    size: usize,
) -> Option<GroundBoxes> {
    let stem = heightmap_file
        .rsplit_once('.')
        .map_or(heightmap_file, |(stem, _)| stem);
    let file = |extension: &str| {
        archive
            .find_file_in(folder::DATA, &format!("{stem}.{extension}"))
            .map(|entry| archive.data(entry))
    };
    GroundBoxes::parse(file("RA0")?, file("RA1")?, file("CL0"), size).ok()
}

/// The level file doesn't name the texture types either. They are the heightmap's name
/// with the extension .TTY, in all 12 archives seen (see `docs/formats/texture_types.md`).
/// Types that can't be read cost the track its surfaces, not the race.
fn read_texture_types(archive: &PodArchive, heightmap_file: &str) -> Option<TextureTypes> {
    let stem = heightmap_file
        .rsplit_once('.')
        .map_or(heightmap_file, |(stem, _)| stem);
    let entry = archive.find_file_in(folder::DATA, &format!("{stem}.TTY"))?;
    TextureTypes::parse(&text(archive.data(entry))).ok()
}

/// Tracks converted for Evolution 2 name the file .SI2 and lay it out as a .SIT; see
/// `docs/formats/situation.md`.
fn situation_entry(archive: &PodArchive) -> Option<&Entry> {
    first_with_extension(archive, folder::WORLD, "sit")
        .or_else(|| first_with_extension(archive, folder::WORLD, "si2"))
}

fn truck_entry(archive: &PodArchive) -> Option<&Entry> {
    first_with_extension(archive, folder::TRUCK, "trk")
}

/// The first file with the extension, preferring those in `folder`.
fn first_with_extension<'a>(
    archive: &'a PodArchive,
    folder: &str,
    extension: &str,
) -> Option<&'a Entry> {
    let mut any = None;
    for entry in archive.entries() {
        if !entry.extension().eq_ignore_ascii_case(extension) {
            continue;
        }
        if entry.folder().eq_ignore_ascii_case(folder) {
            return Some(entry);
        }
        any.get_or_insert(entry);
    }
    any
}

fn find<'a>(archive: &'a PodArchive, folder: &str, file_name: &str) -> Result<&'a Entry, PodError> {
    archive
        .find_file_in(folder, file_name)
        .ok_or_else(|| PodError::MissingFile(file_name.to_string()))
}

/// The text files are in a Windows code page. Only ASCII matters to us, so every byte
/// becomes the character with the same number and nothing can fail.
fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| byte as char).collect()
}

#[cfg(test)]
mod tests {
    use super::archive::tests::build_archive;
    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> PodArchive {
        PodArchive::parse(build_archive("", files)).unwrap()
    }

    #[test]
    fn finds_a_track_file_named_si2_when_there_is_no_sit() {
        let text = b"TRACK.LVL\r\n!Race Track Name\r\nConverted\r\n";
        let converted = archive(&[("WORLD\\TRACK.SI2", text)]);
        assert_eq!(Track::name_in(&converted).as_deref(), Some("Converted"));
    }

    #[test]
    fn lists_and_loads_each_of_several_tracks() {
        let two = archive(&[
            ("WORLD\\A.SIT", b"A.LVL\r\n!Race Track Name\r\nFirst\r\n"),
            ("WORLD\\B.SIT", b"B.LVL\r\n!Race Track Name\r\nSecond\r\n"),
            ("WORLD\\C.SI2", b"C.LVL\r\n!Race Track Name\r\nThird\r\n"),
        ]);
        let files: Vec<&str> = Track::files_in(two.entries())
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(files, ["WORLD\\A.SIT", "WORLD\\B.SIT"]);
        assert_eq!(
            Track::name_from(two.data(Track::files_in(two.entries())[1])),
            "Second"
        );
        // Found, and then stopped by its missing level file, not by picking the first.
        let error = Track::from_file(&two, "WORLD\\B.SIT", &NoBase).unwrap_err();
        assert!(error.to_string().contains("B.LVL"), "{error}");
        assert!(Track::from_file(&two, "D.SIT", &NoBase).is_err());
    }

    #[test]
    fn prefers_a_sit_to_an_si2() {
        let both = archive(&[
            ("WORLD\\B.SI2", b"B.LVL\r\n!Race Track Name\r\nSecond\r\n"),
            ("WORLD\\A.SIT", b"A.LVL\r\n!Race Track Name\r\nFirst\r\n"),
        ]);
        assert_eq!(Track::name_in(&both).as_deref(), Some("First"));
    }

    fn ints(values: &[i32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    /// A model of one vertex at `x` in model units, and an end record.
    fn point(x: i32) -> Vec<u8> {
        ints(&[20, 65536, 2, 0, 1, x, 0, 0, 0])
    }

    /// An animation control file naming `frames`, a second each.
    fn control(frames: &[&str]) -> Vec<u8> {
        let mut bytes = ints(&[0x20, 0, frames.len() as i32, 65536, 0, 0]);
        for frame in frames {
            let mut name = frame.as_bytes().to_vec();
            name.resize(16, 0);
            bytes.extend(name);
        }
        bytes
    }

    #[test]
    fn a_box_that_names_an_animation_control_file_gets_its_first_frame_and_all_frames() {
        let two = ints(&[20, 65536, 2, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
        let control = control(&["A.BIN", "B.BIN"]);
        let mismatched = self::control(&["A.BIN", "TWO.BIN"]);
        let missing = self::control(&["A.BIN", "GONE.BIN"]);
        let (a, b) = (point(256), point(512));
        let archive = archive(&[
            ("MODELS\\ANIM.BIN", &control),
            ("MODELS\\ODD.BIN", &mismatched),
            ("MODELS\\LOST.BIN", &missing),
            ("MODELS\\A.BIN", &a),
            ("MODELS\\B.BIN", &b),
            ("MODELS\\TWO.BIN", &two),
        ]);
        let files = Files::new(&archive, &NoBase);

        let (model, animated) = read_model(&files, "anim.bin").unwrap();
        assert_eq!(model.vertices, [[1.0, 0.0, 0.0]]);
        let animated = animated.unwrap();
        assert_eq!(animated.frames, [[[1.0, 0.0, 0.0]], [[2.0, 0.0, 0.0]]]);
        assert_eq!(animated.animation.seconds_per_frame, 1.0);

        // A plain model has no frames, and a frame that doesn't fit, or isn't there,
        // leaves the first frame still.
        assert!(read_model(&files, "B.BIN").unwrap().1.is_none());
        for still in ["ODD.BIN", "LOST.BIN"] {
            let (model, animated) = read_model(&files, still).unwrap();
            assert_eq!(model.vertices.len(), 1);
            assert!(animated.is_none(), "{still}");
        }
        assert!(read_model(&files, "NONE.BIN").is_none());
    }
}
