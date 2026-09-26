//! Rapier's collider wireframes, off by default and toggled with F1 (`keys::Control::
//! PhysicsDebug`).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::keys::{Control, KeyBindings, just_pressed};

pub struct PhysicsDebugPlugin;

impl Plugin for PhysicsDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierDebugRenderPlugin::default().disabled())
            .init_resource::<KeyBindings>()
            .add_systems(
                Update,
                toggle_physics_debug.run_if(just_pressed(Control::PhysicsDebug)),
            );
    }
}

fn toggle_physics_debug(mut debug: ResMut<DebugRenderContext>) {
    debug.enabled = !debug.enabled;
}
