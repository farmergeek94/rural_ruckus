//! What each kind of weather is, as plain numbers: the sky, the light, how far can be seen,
//! what falls, and how much the tires grip. The rest of the slice only carries these out.
//!
//! The values are the game's own. How Monster Truck Madness 2's weather looked is not
//! measured.

use bevy::prelude::*;

use super::Weather;

/// What the weather is like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Conditions {
    /// The colour of the sky, in sRGB from 0 to 1. Fog is the same colour, so that what is
    /// far off goes into the sky.
    pub(super) sky: Vec3,
    /// How brightly the sun shines, as a share of a clear day's `environment::SUNLIGHT`.
    pub(super) sun: f32,
    /// The colour of the sunlight, in sRGB from 0 to 1.
    pub(super) sun_color: Vec3,
    /// How brightly everything is lit from all round, in candela per square metre. Under
    /// cloud, much of the light comes from the whole sky: shadows go soft and faint.
    pub(super) ambient: f32,
    /// How far can be seen through the air, in metres. `None` is clear to the horizon.
    pub(super) visibility: Option<f32>,
    /// What falls from the sky.
    pub(super) fall: Option<Fall>,
    /// How much the tires grip, as a share of their grip in the dry.
    pub(super) grip: f32,
    /// Whether lightning flashes.
    pub(super) lightning: bool,
}

/// Rain or snow, and how much.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Fall {
    pub(super) kind: FallKind,
    /// How many drops or flakes are in the air round the camera at once.
    pub(super) count: usize,
    /// How fast they fall, in m/s.
    pub(super) speed: f32,
    /// How fast the wind carries them along, in m/s.
    pub(super) wind: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FallKind {
    Rain,
    Snow,
}

/// A clear day's ambient light: Bevy's default.
const CLEAR_AMBIENT: f32 = 80.0;
const WHITE: Vec3 = Vec3::ONE;

impl Weather {
    pub(super) fn conditions(self) -> Conditions {
        let clear = Conditions {
            sky: Vec3::new(0.53, 0.74, 0.93),
            sun: 1.0,
            sun_color: WHITE,
            ambient: CLEAR_AMBIENT,
            visibility: None,
            fall: None,
            grip: 1.0,
            lightning: false,
        };
        match self {
            Weather::Clear => clear,
            Weather::Overcast => Conditions {
                sky: Vec3::new(0.62, 0.66, 0.71),
                sun: 0.35,
                sun_color: Vec3::new(0.92, 0.94, 1.0),
                ambient: 400.0,
                // Hazes the backdrop, 900 m out (see `backdrop.rs`), and doesn't hide it.
                visibility: Some(2500.0),
                ..clear
            },
            Weather::Fog => Conditions {
                sky: Vec3::new(0.74, 0.76, 0.78),
                sun: 0.3,
                sun_color: Vec3::new(0.95, 0.95, 0.95),
                ambient: 500.0,
                visibility: Some(90.0),
                ..clear
            },
            Weather::Rain => Conditions {
                sky: Vec3::new(0.46, 0.5, 0.55),
                sun: 0.22,
                sun_color: Vec3::new(0.88, 0.92, 1.0),
                ambient: 260.0,
                visibility: Some(320.0),
                fall: Some(Fall {
                    kind: FallKind::Rain,
                    count: 6000,
                    speed: 9.0,
                    wind: 2.0,
                }),
                // Wet dirt: the tires slide a little sooner. Monster-truck tires dig through
                // the wet top, so keep it near the dry figure: 0.85 was too slippery to drive.
                grip: 0.93,
                ..clear
            },
            Weather::Storm => Conditions {
                sky: Vec3::new(0.24, 0.26, 0.3),
                sun: 0.1,
                sun_color: Vec3::new(0.8, 0.85, 1.0),
                ambient: 130.0,
                visibility: Some(200.0),
                fall: Some(Fall {
                    kind: FallKind::Rain,
                    count: 8000,
                    speed: 11.0,
                    wind: 6.0,
                }),
                // Wetter than rain, and still well above snow.
                grip: 0.88,
                lightning: true,
            },
            Weather::Snow => Conditions {
                sky: Vec3::new(0.8, 0.82, 0.86),
                sun: 0.3,
                sun_color: Vec3::new(0.95, 0.97, 1.0),
                ambient: 500.0,
                visibility: Some(220.0),
                fall: Some(Fall {
                    kind: FallKind::Snow,
                    count: 8000,
                    speed: 1.2,
                    wind: 1.0,
                }),
                grip: 0.7,
                ..clear
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_weather_is_the_game_as_it_was() {
        let clear = Weather::Clear.conditions();
        assert_eq!(clear.sun, 1.0);
        assert_eq!(clear.grip, 1.0);
        assert_eq!(clear.visibility, None);
        assert_eq!(clear.fall, None);
        assert_eq!(clear.ambient, CLEAR_AMBIENT);
        // The same sky that `environment` sets.
        let sky = crate::environment::SKY.to_srgba();
        assert_eq!(clear.sky, Vec3::new(sky.red, sky.green, sky.blue));
    }

    #[test]
    fn worse_weather_grips_less_and_sees_less_far() {
        for weather in Weather::ALL {
            let conditions = weather.conditions();
            assert!(
                conditions.grip > 0.5 && conditions.grip <= 1.0,
                "{weather:?}"
            );
            assert!(conditions.sun > 0.0 && conditions.sun <= 1.0, "{weather:?}");
        }
        let grip = |weather: Weather| weather.conditions().grip;
        assert!(grip(Weather::Rain) < grip(Weather::Clear));
        assert!(grip(Weather::Storm) < grip(Weather::Rain));
        assert!(grip(Weather::Snow) < grip(Weather::Storm));
        let seen = |weather: Weather| weather.conditions().visibility.unwrap();
        assert!(seen(Weather::Fog) < seen(Weather::Storm));
        assert!(seen(Weather::Storm) < seen(Weather::Rain));
    }

    #[test]
    fn only_rain_and_snow_fall_and_only_a_storm_flashes() {
        for weather in Weather::ALL {
            let conditions = weather.conditions();
            let kind = conditions.fall.map(|fall| fall.kind);
            let expected = match weather {
                Weather::Rain | Weather::Storm => Some(FallKind::Rain),
                Weather::Snow => Some(FallKind::Snow),
                _ => None,
            };
            assert_eq!(kind, expected, "{weather:?}");
            assert_eq!(conditions.lightning, weather == Weather::Storm);
        }
    }
}
