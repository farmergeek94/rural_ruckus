//! Avian's collider wireframes, off by default and toggled with F1 (`keys::Control::
//! PhysicsDebug`). Avian draws them with gizmos, so only an app that can draw has them.

use avian3d::prelude::PhysicsGizmos;
use bevy::prelude::*;

use crate::keys::{Control, KeyBindings, just_pressed};

pub struct PhysicsDebugPlugin;

impl Plugin for PhysicsDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(avian3d::debug_render::PhysicsDebugPlugin);
        app.world_mut()
            .resource_mut::<GizmoConfigStore>()
            .config_mut::<PhysicsGizmos>()
            .0
            .enabled = false;
        app.init_resource::<KeyBindings>().add_systems(
            Update,
            toggle_physics_debug.run_if(just_pressed(Control::PhysicsDebug)),
        );
    }
}

fn toggle_physics_debug(mut store: ResMut<GizmoConfigStore>) {
    let config = store.config_mut::<PhysicsGizmos>().0;
    config.enabled = !config.enabled;
}
