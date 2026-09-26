//! Wet and snowy ground grips the tires less. Every truck's grip is its own grip in the
//! dry, which it keeps in `DryGrip`, times the weather's share. The computer's drivers read
//! the grip from the truck, as the tires do, and so drive to it.

use bevy::prelude::*;

use super::WeatherSettings;
use crate::truck::TruckConfig;

/// On a truck: how much its tires grip in the dry, as it was built.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(super) struct DryGrip {
    grip: f32,
    handbrake_grip: f32,
}

pub(super) fn wet_the_tires(
    mut commands: Commands,
    settings: Res<WeatherSettings>,
    mut trucks: Query<(Entity, &mut TruckConfig, Option<&DryGrip>)>,
) {
    let share = settings.weather.conditions().grip;
    for (truck, mut config, dry) in &mut trucks {
        // The first time a truck is seen, its grip is still its own.
        let dry = match dry {
            Some(dry) => *dry,
            None => {
                let dry = DryGrip {
                    grip: config.grip,
                    handbrake_grip: config.handbrake_grip,
                };
                commands.entity(truck).insert(dry);
                dry
            }
        };
        let (grip, handbrake_grip) = (dry.grip * share, dry.handbrake_grip * share);
        // Only a real change, so that nothing that watches the config sees one every frame.
        if config.grip != grip || config.handbrake_grip != handbrake_grip {
            config.grip = grip;
            config.handbrake_grip = handbrake_grip;
        }
    }
}
