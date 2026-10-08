//! The sky's colour, the sunlight, the light from all round, and fog, as the weather and
//! the time of day say. At dusk the sun is low in the sky, and orange under a clear sky;
//! under cloud, fog, rain or snow, dusk is as dim but grey, as the cloud hides the sunset.
//! At night it is the moon, dim and blue, high up.
//! In a storm, lightning: a flash that lights the sky and the ground, flickers, and fades,
//! every few seconds at random.
//!
//! Fog and the light from all round are put on the race camera (`DistanceFog` and
//! `AmbientLight`), so that the front end's showroom keeps its own light. The water mirrors
//! the sky's colour (`ClearColor`), so it goes grey under grey cloud by itself.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use super::conditions::Conditions;
use super::{ParticleLight, TimeOfDay, Weather, WeatherSettings};
use crate::camera::ChaseCamera;
use crate::environment::{SKY, SUN_FROM, SUNLIGHT, Sun};

/// The light from all round that lights particles fully on its own, in candela per square
/// metre: about the brightest of the weathers'. With the sun's share, a clear day, overcast
/// and fog are full light.
const FULL_AMBIENT: f32 = 500.0;
/// The least light particles have, so that at night what the lamps light is still seen.
const DARKEST_PARTICLES: f32 = 0.06;

/// How brightly the dust and the spray are drawn in `light`, from 0 to 1.
fn particle_level(light: &Light) -> f32 {
    (light.sun / SUNLIGHT + light.ambient / FULL_AMBIENT).clamp(DARKEST_PARTICLES, 1.0)
}

/// What a time of day does to the weather's light. The game's own.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hour {
    /// The colour the sky goes towards, in sRGB from 0 to 1, and how far, from 0 to 1.
    sky: Vec3,
    sky_share: f32,
    /// What the sunlight and the light from all round are multiplied by.
    sun: f32,
    ambient: f32,
    /// What the sun's colour is multiplied by, in sRGB from 0 to 1.
    sun_color: Vec3,
    /// Where the sun (or the moon) is: the way to it from the middle of the track.
    sun_from: Vec3,
}

/// A colour as the grey of the same brightness, in sRGB from 0 to 1: its luminance, with
/// the weights of sRGB's primaries.
fn grey(color: Vec3) -> Vec3 {
    Vec3::splat(color.dot(Vec3::new(0.2126, 0.7152, 0.0722)))
}

impl TimeOfDay {
    /// What this time of day does to the light of `weather`.
    fn hour(self, weather: Weather) -> Hour {
        match self {
            TimeOfDay::Day => Hour {
                sky: Vec3::ZERO,
                sky_share: 0.0,
                sun: 1.0,
                ambient: 1.0,
                sun_color: Vec3::ONE,
                sun_from: SUN_FROM,
            },
            // About 10 degrees up, in the west. Lower throws longer shadows. The sunset's
            // orange only under a clear sky: any other weather keeps the dimming, in grey.
            TimeOfDay::Dusk => {
                let sunset = |color: Vec3| {
                    if weather == Weather::Clear {
                        color
                    } else {
                        grey(color)
                    }
                };
                Hour {
                    sky: sunset(Vec3::new(0.85, 0.5, 0.35)),
                    sky_share: 0.6,
                    sun: 0.4,
                    ambient: 0.4,
                    sun_color: sunset(Vec3::new(1.0, 0.62, 0.38)),
                    sun_from: Vec3::new(-100.0, 18.0, 30.0),
                }
            }
            // Enough moonlight to see the course's shape by; the lamps light the rest.
            TimeOfDay::Night => Hour {
                sky: Vec3::new(0.02, 0.03, 0.07),
                sky_share: 0.95,
                sun: 0.04,
                ambient: 0.08,
                sun_color: Vec3::new(0.6, 0.7, 1.0),
                sun_from: Vec3::new(-40.0, 100.0, -60.0),
            },
        }
    }
}

/// The colour of the sky in a flash of lightning, in sRGB from 0 to 1, and how far towards
/// it a full flash takes the sky.
const FLASH_SKY: Vec3 = Vec3::new(0.85, 0.87, 0.95);
const FLASH_SKY_SHARE: f32 = 0.8;
/// How much a full flash adds to the sunlight, as a share of a clear day's, and to the
/// light from all round, in candela per square metre.
const FLASH_SUN: f32 = 1.0;
const FLASH_AMBIENT: f32 = 1500.0;
/// The shortest and the longest time from one flash to the next, in seconds.
const FLASH_GAP: [f32; 2] = [5.0, 14.0];

/// What the storm remembers: when the last flash began and when the next will, in seconds
/// of the game's time.
pub(super) struct Storm {
    last: f32,
    next: f32,
    random: Random,
}

impl Default for Storm {
    fn default() -> Self {
        Self {
            last: f32::NEG_INFINITY,
            next: FLASH_GAP[0],
            random: Random::default(),
        }
    }
}

type RaceCameras<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static mut DistanceFog>,
        Option<&'static mut AmbientLight>,
    ),
    With<ChaseCamera>,
>;

#[expect(
    clippy::too_many_arguments,
    reason = "it lights everything the weather lights"
)]
pub(super) fn light_the_sky(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<WeatherSettings>,
    sky: Option<ResMut<ClearColor>>,
    mut suns: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut cameras: RaceCameras,
    particle_light: Option<ResMut<ParticleLight>>,
    mut storm: Local<Storm>,
) {
    let conditions = settings.weather.conditions();
    let now = time.elapsed_secs();
    let flash = if conditions.lightning {
        if now >= storm.next {
            storm.last = now;
            storm.next = now + storm.random.between(FLASH_GAP[0], FLASH_GAP[1]);
        }
        flash(now - storm.last)
    } else {
        0.0
    };
    let hour = settings.time_of_day.hour(settings.weather);
    let light = lit(&conditions, &hour, flash);

    if let Some(mut sky) = sky
        && sky.0 != light.sky
    {
        sky.0 = light.sky;
    }
    if let Some(mut particles) = particle_light {
        particles.set_if_neq(ParticleLight(particle_level(&light)));
    }
    let placed = Transform::from_translation(hour.sun_from).looking_at(Vec3::ZERO, Vec3::Y);
    for (mut sun, mut transform) in &mut suns {
        if sun.illuminance != light.sun || sun.color != light.sun_color {
            sun.illuminance = light.sun;
            sun.color = light.sun_color;
        }
        transform.set_if_neq(placed);
    }
    for (camera, fog, ambient) in &mut cameras {
        match (conditions.visibility, fog) {
            (Some(visibility), Some(mut fog)) => {
                fog.color = light.sky;
                fog.falloff = FogFalloff::from_visibility_squared(visibility);
            }
            (Some(visibility), None) => {
                commands.entity(camera).insert(DistanceFog {
                    color: light.sky,
                    directional_light_color: Color::NONE,
                    falloff: FogFalloff::from_visibility_squared(visibility),
                    ..default()
                });
            }
            (None, Some(_)) => {
                commands.entity(camera).remove::<DistanceFog>();
            }
            (None, None) => {}
        }
        match ambient {
            Some(mut ambient) => {
                if ambient.brightness != light.ambient {
                    ambient.brightness = light.ambient;
                }
            }
            None => {
                commands.entity(camera).insert(AmbientLight {
                    brightness: light.ambient,
                    ..default()
                });
            }
        }
    }
}

/// The front end is under a clear sky, whatever the weather of the last race.
pub(super) fn clear_the_sky(sky: Option<ResMut<ClearColor>>) {
    if let Some(mut sky) = sky {
        sky.0 = SKY;
    }
}

/// How the sky and the ground are lit.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Light {
    sky: Color,
    /// In lux.
    sun: f32,
    sun_color: Color,
    /// In candela per square metre.
    ambient: f32,
}

/// The light of `conditions` at `hour`, with a flash of lightning as bright as `flash`
/// (0 to 1).
fn lit(conditions: &Conditions, hour: &Hour, flash: f32) -> Light {
    let sky = conditions
        .sky
        .lerp(hour.sky, hour.sky_share)
        .lerp(FLASH_SKY, flash * FLASH_SKY_SHARE);
    let sun_color = (conditions.sun_color * hour.sun_color).lerp(Vec3::ONE, flash);
    Light {
        sky: Color::srgb(sky.x, sky.y, sky.z),
        // The dimmer of the two: a storm at night is as dark as a clear night, not darker
        // again by the storm's share. Multiplied, a storm at dusk was nearly black.
        sun: SUNLIGHT * (conditions.sun.min(hour.sun) + flash * FLASH_SUN),
        sun_color: Color::srgb(sun_color.x, sun_color.y, sun_color.z),
        ambient: conditions.ambient * hour.ambient + flash * FLASH_AMBIENT,
    }
}

/// How bright a flash of lightning is `since` seconds after it began, from 0 to 1: a bright
/// strike, a moment of near dark, a second strike, and a glow that dies away.
fn flash(since: f32) -> f32 {
    match since {
        since if since < 0.0 => 0.0,
        since if since < 0.06 => 1.0,
        since if since < 0.14 => 0.2,
        since if since < 0.22 => 0.8,
        since => {
            let glow = 0.8 * (-(since - 0.22) * 8.0).exp();
            if glow < 0.01 { 0.0 } else { glow }
        }
    }
}

/// A small, quick source of numbers that only have to look random (xorshift).
struct Random(u64);

impl Default for Random {
    fn default() -> Self {
        Self(0x9E37_79B9_7F4A_7C15)
    }
}

impl Random {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::Weather;

    #[test]
    fn a_flash_strikes_twice_then_dies_away() {
        assert_eq!(flash(-1.0), 0.0);
        assert_eq!(flash(0.0), 1.0);
        assert!(flash(0.1) < flash(0.18));
        assert!(flash(0.3) < flash(0.22));
        assert_eq!(flash(2.0), 0.0);
    }

    #[test]
    fn a_clear_day_is_lit_as_the_environment_lights_it() {
        let light = lit(
            &Weather::Clear.conditions(),
            &TimeOfDay::Day.hour(Weather::Clear),
            0.0,
        );
        assert_eq!(light.sky, SKY);
        assert_eq!(light.sun, SUNLIGHT);
        assert_eq!(light.sun_color.to_linear(), Color::WHITE.to_linear());
    }

    #[test]
    fn dusk_is_darker_than_day_and_night_darker_still() {
        let clear = Weather::Clear.conditions();
        let [day, dusk, night] =
            TimeOfDay::ALL.map(|time| lit(&clear, &time.hour(Weather::Clear), 0.0));
        assert!(day.sun > dusk.sun && dusk.sun > night.sun);
        assert!(day.ambient > dusk.ambient && dusk.ambient > night.ambient);
        let brightness = |color: Color| color.to_srgba().red + color.to_srgba().green;
        assert!(brightness(night.sky) < brightness(dusk.sky));
        // The sun is lower at dusk than by day.
        let height = |time: TimeOfDay| time.hour(Weather::Clear).sun_from.normalize().y;
        assert!(height(TimeOfDay::Dusk) < height(TimeOfDay::Day));
    }

    #[test]
    fn dusk_is_orange_only_under_a_clear_sky() {
        // How much redder than blue a colour is.
        let warmth = |color: Color| color.to_srgba().red - color.to_srgba().blue;
        let dusk =
            |weather: Weather| lit(&weather.conditions(), &TimeOfDay::Dusk.hour(weather), 0.0);
        let clear = dusk(Weather::Clear);
        assert!(warmth(clear.sky) > 0.1 && warmth(clear.sun_color) > 0.3);
        for weather in Weather::ALL.into_iter().filter(|w| *w != Weather::Clear) {
            let light = dusk(weather);
            assert!(warmth(light.sky) <= 0.0, "{weather:?}: {:?}", light.sky);
            assert!(
                warmth(light.sun_color) <= 0.0,
                "{weather:?}: {:?}",
                light.sun_color
            );
            // As dim as a clear dusk: only the colour goes.
            let day = lit(&weather.conditions(), &TimeOfDay::Day.hour(weather), 0.0);
            assert!(light.ambient < day.ambient);
        }
    }

    #[test]
    fn dust_is_dim_at_dusk_and_dark_at_night() {
        let clear = Weather::Clear.conditions();
        let level = |time: TimeOfDay| particle_level(&lit(&clear, &time.hour(Weather::Clear), 0.0));
        assert_eq!(level(TimeOfDay::Day), 1.0);
        assert!(level(TimeOfDay::Dusk) < 0.6);
        assert!(level(TimeOfDay::Night) < 0.15);
    }

    #[test]
    fn a_storm_is_no_darker_at_night_than_a_clear_night() {
        let night = TimeOfDay::Night.hour(Weather::Clear);
        let clear = lit(&Weather::Clear.conditions(), &night, 0.0);
        let storm = lit(&Weather::Storm.conditions(), &night, 0.0);
        assert!(storm.sun >= clear.sun);
        let dusk = TimeOfDay::Dusk.hour(Weather::Storm);
        let storm_dusk = lit(&Weather::Storm.conditions(), &dusk, 0.0);
        assert!(storm_dusk.sun >= SUNLIGHT * Weather::Storm.conditions().sun);
    }

    #[test]
    fn lightning_lights_the_storm_up() {
        let storm = Weather::Storm.conditions();
        let day = TimeOfDay::Day.hour(Weather::Clear);
        let dark = lit(&storm, &day, 0.0);
        let flash = lit(&storm, &day, 1.0);
        assert!(flash.sun > dark.sun);
        assert!(flash.ambient > dark.ambient);
        let brightness = |color: Color| color.to_srgba().red + color.to_srgba().green;
        assert!(brightness(flash.sky) > brightness(dark.sky));
    }
}
