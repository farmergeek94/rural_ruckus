//! The weather of a race: clear, overcast, fog, rain, a storm or snow.
//!
//! - `conditions`: what each kind of weather is, as plain numbers.
//! - `sky`: the sky's colour, how brightly the sun and the sky light the ground, and fog
//!   that hides what is far off in the sky's colour. In a storm, lightning flashes.
//! - `fall`: the rain or snow round the camera. One mesh of many small squares, made once,
//!   which `fall.wgsl` moves on the graphics card. The drops keep to their places in the world as the camera goes through
//!   them, and what goes out of a box round the camera comes in again at the other side.
//! - `cover`: what keeps the rain and snow off round the camera, such as a bridge or a
//!   roof, found once for each square metre by a ray cast down.
//! - `grip`: wet or snowy ground grips the tires less, every truck alike.
//!
//! The time of day (`TimeOfDay`) is part of the weather's settings: dusk lowers the sun and,
//! under a clear sky, turns it orange, night leaves dim blue moonlight, and both, like rain and storms, turn on the trucks' lamps
//! (`truck::TruckLamps`). F8 goes on to the next time in a race.
//!
//! The weather is chosen with `WeatherSettings`, on the options screen, or with F7 in a
//! race, which goes on to the next kind. Or it is left to chance: a new kind, and a new
//! time of day, picked at random as each race begins. Clear is the game as it was before there was
//! weather. The values are the game's own: Monster Truck Madness 2's weather is not
//! measured.
//!
//! Uses the `environment` slice for the sun, the `camera` slice for the race camera, the
//! `track` slice for the ground that rain and snow stop at, and the `truck` slice for the
//! tires. Only the grip works in an app that cannot draw.

mod conditions;
mod cover;
mod fall;
mod grip;
mod sky;

use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;

use crate::camera::CameraSystems;
use crate::game_state::GameState;
use crate::keys::{Control, KeyBindings, just_pressed};
use crate::truck::TruckLamps;

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        // `TrackPlugin` and `TruckPlugin`, which this slice needs, have made sure of the state.
        app.init_resource::<WeatherSettings>()
            .init_resource::<ParticleLight>()
            .init_resource::<TruckLamps>()
            .init_resource::<KeyBindings>()
            .add_systems(OnEnter(GameState::Racing), pick_the_weather)
            .add_systems(
                Update,
                (
                    grip::wet_the_tires,
                    light_the_lamps,
                    next_weather.run_if(just_pressed(Control::NextWeather)),
                    next_time.run_if(just_pressed(Control::NextTimeOfDay)),
                )
                    .run_if(in_state(GameState::Racing)),
            );
        // The rest is only a look.
        if !app.is_plugin_added::<PbrPlugin>() {
            return;
        }
        // In `src/shaders`, with the game's other shaders: `embedded_asset!` takes only a
        // path below this file's folder.
        app.world_mut()
            .resource_mut::<EmbeddedAssetRegistry>()
            .insert_asset(
                PathBuf::from("src/shaders/fall.wgsl"),
                Path::new(fall::SHADER_PATH.trim_start_matches("embedded://")),
                include_bytes!("../shaders/fall.wgsl").as_slice(),
            );
        app.add_plugins(MaterialPlugin::<fall::FallMaterial>::default())
            .add_systems(Startup, fall::make_looks)
            .add_systems(OnExit(GameState::Racing), sky::clear_the_sky)
            .add_systems(
                Update,
                (
                    sky::light_the_sky,
                    // Round the camera as it is placed for this frame.
                    (
                        fall::start_falling,
                        fall::follow_the_camera,
                        cover::find_cover,
                    )
                        .chain()
                        .after(CameraSystems::Place),
                    fall::light_the_fall
                        .after(sky::light_the_sky)
                        .after(fall::start_falling),
                )
                    .run_if(in_state(GameState::Racing)),
            );
    }
}

/// Which weather it is, and the time of day. Insert it before adding `WeatherPlugin`, or
/// change it at any time.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherSettings {
    pub weather: Weather,
    /// Whether each race's weather and time of day are picked at random as it begins, into
    /// `weather` and `time_of_day`.
    pub random: bool,
    pub time_of_day: TimeOfDay,
}

/// How brightly what is not lit is drawn (the dust, the spray, the rain and the snow), from
/// 0 (black) to 1 (its own colours, as in full daylight), as the weather and the time of
/// day light the world.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ParticleLight(pub f32);

impl Default for ParticleLight {
    fn default() -> Self {
        Self(1.0)
    }
}

/// When in the day a race is. The game's own light: MTM2 offered a dusk and a night sky
/// (see `docs/formats/level.md`), but how it lit the ground under them is not measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeOfDay {
    /// The sun high, as the game was.
    #[default]
    Day,
    /// The sun low, and orange under a clear sky, and the trucks' lamps on.
    Dusk,
    /// Dim blue moonlight, and the trucks' lamps on.
    Night,
}

impl TimeOfDay {
    /// Every time, in the order F8 and the options screen go through them.
    pub const ALL: [TimeOfDay; 3] = [TimeOfDay::Day, TimeOfDay::Dusk, TimeOfDay::Night];

    /// What the player sees it called, and what the command line knows it by.
    pub fn name(self) -> &'static str {
        match self {
            TimeOfDay::Day => "Day",
            TimeOfDay::Dusk => "Dusk",
            TimeOfDay::Night => "Night",
        }
    }

    /// The time called `name`, in any case.
    pub fn named(name: &str) -> Option<TimeOfDay> {
        TimeOfDay::ALL
            .into_iter()
            .find(|time| time.name().eq_ignore_ascii_case(name))
    }

    /// Whether it is dark enough for the trucks' lamps.
    pub fn dark(self) -> bool {
        self != TimeOfDay::Day
    }

    /// The time after this one, round to the first after the last.
    pub fn next(self) -> TimeOfDay {
        let at = TimeOfDay::ALL
            .iter()
            .position(|&each| each == self)
            .unwrap_or(0);
        TimeOfDay::ALL[(at + 1) % TimeOfDay::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weather {
    /// Sunshine and a blue sky: Monster Truck Madness 2's own.
    #[default]
    Clear,
    /// Grey cloud and a soft light.
    Overcast,
    /// Thick fog: the course comes out of it a stretch at a time.
    Fog,
    /// Rain, and wet ground that grips less.
    Rain,
    /// Heavy rain on the wind, a dark sky, and lightning.
    Storm,
    /// Falling snow, ground that grips least of all, and the water frozen to ice.
    Snow,
}

impl Weather {
    /// Every kind, in the order F7 and the options screen go through them.
    pub const ALL: [Weather; 6] = [
        Weather::Clear,
        Weather::Overcast,
        Weather::Fog,
        Weather::Rain,
        Weather::Storm,
        Weather::Snow,
    ];

    /// What the player sees it called, and what the command line and the store know it by.
    pub fn name(self) -> &'static str {
        match self {
            Weather::Clear => "Clear",
            Weather::Overcast => "Overcast",
            Weather::Fog => "Fog",
            Weather::Rain => "Rain",
            Weather::Storm => "Storm",
            Weather::Snow => "Snow",
        }
    }

    /// The weather called `name`, in any case.
    pub fn named(name: &str) -> Option<Weather> {
        Weather::ALL
            .into_iter()
            .find(|weather| weather.name().eq_ignore_ascii_case(name))
    }

    /// Whether the ground is dry enough to raise dust. Fog, rain and snow wet it.
    pub fn dusty(self) -> bool {
        matches!(self, Weather::Clear | Weather::Overcast)
    }

    /// Whether it is cold enough for the track's water to be ice, which trucks drive on.
    pub fn freezes_water(self) -> bool {
        self == Weather::Snow
    }

    /// The kind after this one, round to the first after the last.
    pub fn next(self) -> Weather {
        let at = Weather::ALL
            .iter()
            .position(|&each| each == self)
            .unwrap_or(0);
        Weather::ALL[(at + 1) % Weather::ALL.len()]
    }
}

/// Whether the trucks' lamps are on: at dusk, at night, and in rain and storms.
fn lamps_on(settings: &WeatherSettings) -> bool {
    settings.time_of_day.dark() || matches!(settings.weather, Weather::Rain | Weather::Storm)
}

/// A new kind of weather and time of day for a race, when they are left to chance.
fn pick_the_weather(mut settings: ResMut<WeatherSettings>) {
    if !settings.random {
        return;
    }
    // Only has to differ from one race to the next.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos() as u64);
    (settings.weather, settings.time_of_day) = pick(seed);
    info!(
        "Weather: {}, {}",
        settings.weather.name(),
        settings.time_of_day.name()
    );
}

/// The weather and the time of day that `seed` picks, every kind as likely as another and
/// every time as likely as another, whatever the kind.
fn pick(seed: u64) -> (Weather, TimeOfDay) {
    // Stirred (splitmix64), so that seeds close together pick different kinds.
    let mut mixed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 31;
    let kinds = Weather::ALL.len() as u64;
    let weather = Weather::ALL[(mixed % kinds) as usize];
    // What is left once the kind is taken out, so that the two don't go together.
    let time = TimeOfDay::ALL[(mixed / kinds % TimeOfDay::ALL.len() as u64) as usize];
    (weather, time)
}

fn next_weather(mut settings: ResMut<WeatherSettings>) {
    settings.weather = settings.weather.next();
    info!("Weather: {}", settings.weather.name());
}

fn next_time(mut settings: ResMut<WeatherSettings>) {
    settings.time_of_day = settings.time_of_day.next();
    info!("Time of day: {}", settings.time_of_day.name());
}

/// The trucks' lamps shine when it is dark, and in rain and storms. The cones of their
/// beams are seen only at night; otherwise a beam is only the light it casts.
fn light_the_lamps(settings: Res<WeatherSettings>, mut lamps: ResMut<TruckLamps>) {
    let wanted = TruckLamps {
        lit: lamps_on(&settings),
        cones: settings.time_of_day == TimeOfDay::Night,
    };
    if *lamps != wanted {
        *lamps = wanted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_known_by_its_name() {
        for weather in Weather::ALL {
            assert_eq!(Weather::named(weather.name()), Some(weather));
        }
        assert_eq!(Weather::named("snow"), Some(Weather::Snow));
        assert_eq!(Weather::named("hail"), None);
    }

    #[test]
    fn chance_picks_every_kind_and_every_time_about_as_often() {
        // Every pair of a kind and a time, so that neither leans on the other.
        let mut counts = [[0; TimeOfDay::ALL.len()]; Weather::ALL.len()];
        for seed in 0..18000 {
            let (weather, time) = pick(seed);
            let kind = Weather::ALL.iter().position(|&w| w == weather).unwrap();
            let when = TimeOfDay::ALL.iter().position(|&t| t == time).unwrap();
            counts[kind][when] += 1;
        }
        for count in counts.into_iter().flatten() {
            assert!((800..1200).contains(&count), "{counts:?}");
        }
    }

    #[test]
    fn only_dry_weather_raises_dust() {
        let dusty: Vec<Weather> = Weather::ALL.into_iter().filter(|w| w.dusty()).collect();
        assert_eq!(dusty, [Weather::Clear, Weather::Overcast]);
    }

    #[test]
    fn every_time_of_day_is_known_by_its_name_and_only_day_is_light() {
        for time in TimeOfDay::ALL {
            assert_eq!(TimeOfDay::named(time.name()), Some(time));
            assert_eq!(time.dark(), time != TimeOfDay::Day);
        }
        assert_eq!(TimeOfDay::Night.next(), TimeOfDay::Day);
    }

    #[test]
    fn the_lamps_are_on_when_dark_and_in_rain_and_storms() {
        let with = |weather, time_of_day| {
            lamps_on(&WeatherSettings {
                weather,
                time_of_day,
                ..default()
            })
        };
        assert!(!with(Weather::Clear, TimeOfDay::Day));
        assert!(with(Weather::Rain, TimeOfDay::Day));
        assert!(with(Weather::Storm, TimeOfDay::Day));
        assert!(with(Weather::Clear, TimeOfDay::Dusk));
        assert!(!with(Weather::Snow, TimeOfDay::Day));
    }

    #[test]
    fn only_snow_freezes_the_water() {
        let freezing: Vec<Weather> = Weather::ALL
            .into_iter()
            .filter(|w| w.freezes_water())
            .collect();
        assert_eq!(freezing, [Weather::Snow]);
    }

    #[test]
    fn next_goes_through_every_kind_and_back_round() {
        let mut weather = Weather::Clear;
        for expected in Weather::ALL.iter().skip(1) {
            weather = weather.next();
            assert_eq!(weather, *expected);
        }
        assert_eq!(weather.next(), Weather::Clear);
    }
}
