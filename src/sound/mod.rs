//! The sounds of the game: for now the trucks' engines, heard.
//!
//! The `truck` slice decides what an engine does (its `Engine`: the speed, the fuel,
//! whether it runs); this slice decides what that sounds like. Every truck sounds the base
//! game's one engine: three loops that the gear chooses between (`loops`, with
//! `pod_import` finding them in `SOUND.POD`). Without the base game the engines are
//! silent. (A synthesized diesel was tried and set aside on 2026-10-10, by the user's
//! request, to stay with the base game's sound for now.)
//!
//! The audio thread asks for real-time priority (cpal's `audio_thread_priority` feature,
//! in `Cargo.toml`): Bevy opens the output with rodio's default of a 2048-frame period in
//! a 4096-frame buffer (**reference**: `rodio::DeviceSinkBuilder::from_device` and cpal's
//! ALSA `set_hw_params_from_format`), 43 ms in which the thread must be scheduled again,
//! which the game's own threads at full load cannot be trusted to leave it. On Linux the
//! request goes to RTKit; refused, cpal prints `Failed to promote audio thread to
//! real-time priority` on stderr and plays on as before.
//!
//! The sound comes from the race camera (`camera::ChaseCamera`), which this slice gives a
//! `SpatialListener`: an engine within `FULL_VOLUME_WITHIN` of it is heard in full, and
//! one further off falls away with the square of its distance. Every engine is silent
//! while the game is paused.
//!
//! Without an audio device, or in an app without Bevy's `AudioPlugin`, the slice does
//! nothing but hold its settings (`SoundSettings`).

mod loops;
mod pod_import;

use bevy::audio::{AddAudioSource, AudioPlugin};
use bevy::prelude::*;

use crate::base_game::BaseGame;
use crate::camera::ChaseCamera;
use loops::LoopSound;

/// Within how far of the camera an engine is heard in full, in metres. Further off it
/// falls away with the square of the distance: at twice this, to a quarter. Larger hears
/// the other trucks from further away.
const FULL_VOLUME_WITHIN: f32 = 10.0;

/// How far apart the listener's ears are, in metres. Only the ratio of the distances to
/// each ear pans a sound, so this sets how sharply an engine to one side is heard on it.
const EAR_GAP: f32 = 2.0;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundSettings>();
        if !app.is_plugin_added::<AudioPlugin>() {
            return;
        }
        app.add_audio_source::<LoopSound>().add_systems(
            Update,
            (
                loops::load_loops.run_if(resource_exists_and_changed::<BaseGame>),
                listen_from_the_camera,
                loops::start_engines,
                loops::tune_engines,
            )
                .chain(),
        );
    }
}

/// How the engines are heard. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SoundSettings {
    /// From 0 (silent) to 1.
    pub volume: f32,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self { volume: 0.7 }
    }
}

/// On a drawn truck whose engine has been started.
#[derive(Component)]
struct EngineStarted;

/// Hears the race from its camera.
fn listen_from_the_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<ChaseCamera>, Without<SpatialListener>)>,
) {
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(SpatialListener::new(EAR_GAP));
    }
}
