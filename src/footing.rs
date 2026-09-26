//! What the ground is under each tire: on ice, a tire grips less.
//!
//! Each physics step, before the tires' forces are worked out, every truck's `GroundGrip`
//! is set from the track's footing (`TrackData::footing_at`) under the bottom of each tire
//! as it hangs at rest. Only the grip changes, so a tire on ice slides, spins and brakes
//! late, and a truck with two wheels on it pulls towards the side that grips.
//!
//! The footing is the ground's. A tire on a bridge or a ground box over ice gets the ice's
//! grip: MTM2's texture types say nothing of those.
//!
//! In weather that freezes the water (see `weather::Weather::freezes_water`), a tire over
//! the water, and no higher above it than `ON_THE_ICE`, is on ice too: the frozen water
//! that the `water` slice makes solid. A tire on a bridge over it keeps the bridge's grip.
//!
//! Uses the `track` slice for the ground, the `truck` slice for the trucks, and the
//! `weather` slice, if it is there, for whether the water is frozen. How much MTM2's ice
//! grips is not measured: `ICE_GRIP` is the game's own.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::track::{Footing, Track, TrackData};
use crate::truck::{GroundGrip, Truck, TruckConfig, TruckSystems};
use crate::weather::WeatherSettings;

/// How much of a tire's grip ice leaves it, from 0 to 1. With the built-in truck's grip,
/// about a third of what snow weather leaves on firm ground. Lower slides more; at 0 a
/// truck on ice could neither go, turn nor stop.
const ICE_GRIP: f32 = 0.35;
/// How far above frozen water the bottom of a tire, as it hangs at rest, may be and still
/// be on the ice, in metres. A tire pressed onto the ice is a little below its rest; a
/// bridge over the water is well above. In the air, grip does nothing.
const ON_THE_ICE: f32 = 1.0;

pub struct FootingPlugin;

impl Plugin for FootingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            feel_the_ground
                .before(TruckSystems::Drive)
                .before(PhysicsSet::SyncBackend),
        );
    }
}

fn feel_the_ground(
    track: Option<Res<Track>>,
    weather: Option<Res<WeatherSettings>>,
    mut trucks: Query<(&Transform, &TruckConfig, &mut GroundGrip), With<Truck>>,
) {
    let Some(track) = track else {
        return;
    };
    // Without the weather, the water is never frozen.
    let frozen = weather.is_some_and(|weather| weather.weather.freezes_water());
    for (transform, config, mut ground_grip) in &mut trucks {
        let down = transform.down().as_vec3();
        let grip = config.wheel_rest.map(|rest| {
            let bottom = transform.transform_point(rest) + down * config.wheel_radius;
            grip_of(footing_under(&track, bottom, frozen))
        });
        // Only a real change, so that nothing that watches it sees one every step.
        if ground_grip.0 != grip {
            ground_grip.0 = grip;
        }
    }
}

/// What is under the bottom of a tire at `bottom`, in the world, when the water is
/// `frozen` or not.
fn footing_under(track: &TrackData, bottom: Vec3, frozen: bool) -> Footing {
    let on_frozen_water = frozen
        && track.water_level.is_some_and(|level| {
            track.heights.height_at(bottom.x, bottom.z) < level && bottom.y < level + ON_THE_ICE
        });
    if on_frozen_water {
        Footing::Ice
    } else {
        track.footing_at(bottom.x, bottom.z)
    }
}

/// How much of a tire's grip the ground leaves it.
fn grip_of(footing: Footing) -> f32 {
    match footing {
        Footing::Firm => 1.0,
        Footing::Ice => ICE_GRIP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ice_grips_less_than_firm_ground_but_not_nothing() {
        assert_eq!(grip_of(Footing::Firm), 1.0);
        assert!(grip_of(Footing::Ice) > 0.0 && grip_of(Footing::Ice) < 1.0);
    }

    #[test]
    fn frozen_water_is_ice_under_a_tire_on_it_and_not_above() {
        let mut track = crate::track::builtin_track();
        let (x, z) = (0.0, 0.0);
        let ground = track.heights.height_at(x, z);
        let level = ground + 2.0;
        track.water_level = Some(level);
        let on_it = Vec3::new(x, level, z);
        assert_eq!(footing_under(&track, on_it, true), Footing::Ice);
        assert_eq!(footing_under(&track, on_it, false), Footing::Firm);
        // On a bridge over it.
        let over_it = on_it + Vec3::Y * (ON_THE_ICE + 1.0);
        assert_eq!(footing_under(&track, over_it, true), Footing::Firm);
        // Where the ground is above the water, the ground's own.
        track.water_level = Some(ground - 2.0);
        let on_land = Vec3::new(x, ground, z);
        assert_eq!(footing_under(&track, on_land, true), Footing::Firm);
    }
}
