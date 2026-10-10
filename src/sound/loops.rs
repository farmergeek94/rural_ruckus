//! The base game's engine, as its three loops.
//!
//! The base game has one engine (no truck file names an engine sound): three loops, idle,
//! M1 and M2, which `pod_import` finds in `SOUND.POD`. The gear chooses the loop
//! (`loop_for`), as the user describes MTM2: M1 is first gear or reverse pulling away, M2
//! every other gear, and idle the engine idling, in neutral or standing in first or
//! reverse with no throttle. Within its loop the engine's speed (`truck::Engine::revs`)
//! sets the pitch (`loop_speed`), so the engine climbs through each gear and drops at each
//! change up. Each truck plays all three loops at once, from where it is drawn
//! (`TruckVisual`), and a change of loop fades one out and the other in (`FADE_TIME`). A
//! stalled engine is silent, and so is every engine while the game is paused.
//!
//! How MTM2 itself chose and pitched the loops is not known (see
//! `docs/formats/sound.md`). The figures here are the game's own, and want listening to.

use std::sync::Arc;

use bevy::audio::{ChannelCount, Decodable, SampleRate, Source, SpatialScale, Volume};
use bevy::prelude::*;

use super::{EngineStarted, FULL_VOLUME_WITHIN, SoundSettings, pod_import};
use crate::base_game::BaseGame;
use crate::truck::{Engine, Gear, Gearbox, TruckInput, TruckVisual};

/// The loops, by index: in `LoopHandles`, `EngineVoice::layer` and `SPEEDS`.
const IDLE: usize = 0;
const M1: usize = 1;
const M2: usize = 2;

/// How fast each loop is played with the engine at no speed and at its rev limit, as a
/// share of the speed it was recorded at, idle, M1 and M2; between the two, in step with
/// the engine's speed. The idle loop's top is for revving in neutral. Wider sweeps the
/// pitch further through each gear. A first guess, which wants listening to.
const SPEEDS: [(f32, f32); 3] = [(1.0, 2.0), (0.8, 1.5), (0.8, 1.5)];

/// The engine's speed, as a share of its rev limit, under which an engine in first or
/// reverse idles (the idle loop): standing still with no throttle. The slipping clutch
/// revs it past this as soon as the throttle is pressed.
const IDLE_REVS: f32 = 0.05;

/// How long a change of loop takes, in seconds: one fades out and the other in over this
/// time. Shorter is more abrupt; longer blurs a change of gear.
const FADE_TIME: f32 = 0.15;

/// How loud an engine is with the throttle off, as a share of how loud it is at full
/// throttle. Lower makes the throttle more audible.
const OFF_THROTTLE: f32 = 0.6;

/// How loud an engine is at full throttle, at full `SoundSettings::volume`, as a share of
/// its recording. Under 1, so that eight engines near the camera do not clip.
const LOUDNESS: f32 = 0.5;

/// The engine's three loops, as plain data.
#[derive(Clone, Debug)]
pub(super) struct EngineLoops {
    pub(super) idle: Loop,
    pub(super) m1: Loop,
    pub(super) m2: Loop,
}

/// A sound that repeats without a break.
#[derive(Clone, Debug)]
pub(super) struct Loop {
    /// Samples a second.
    pub(super) sample_rate: u32,
    /// One channel, from -1 to 1.
    pub(super) samples: Arc<[f32]>,
}

/// A loop, for the audio player.
#[derive(Asset, TypePath, Clone, Debug)]
pub(super) struct LoopSound(Loop);

impl Decodable for LoopSound {
    type Decoder = LoopDecoder;

    fn decoder(&self) -> Self::Decoder {
        LoopDecoder {
            samples: self.0.samples.clone(),
            sample_rate: SampleRate::new(self.0.sample_rate).unwrap_or(SampleRate::MIN),
            at: 0,
        }
    }
}

/// A loop's samples, over and over, never ending.
pub(super) struct LoopDecoder {
    samples: Arc<[f32]>,
    sample_rate: SampleRate,
    at: usize,
}

impl Iterator for LoopDecoder {
    type Item = bevy::audio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = *self.samples.get(self.at)?;
        self.at = (self.at + 1) % self.samples.len();
        Some(sample)
    }
}

impl Source for LoopDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// The loops, ready to play: idle, M1 and M2. Missing while the base game has no
/// engine sounds.
#[derive(Resource)]
pub(super) struct LoopHandles([Handle<LoopSound>; 3]);

/// One of a truck's three loops, on an entity of its own beside the drawn truck.
#[derive(Component)]
pub(super) struct EngineVoice {
    /// The truck (`truck::Truck`) whose engine it is.
    truck: Entity,
    /// Which loop: `IDLE`, `M1` or `M2`.
    layer: usize,
    /// How far it is faded in, from 0 to 1.
    share: f32,
}

/// Finds the loops in the base game, whenever the base game is opened.
pub(super) fn load_loops(
    mut commands: Commands,
    base: Res<BaseGame>,
    mut sounds: ResMut<Assets<LoopSound>>,
) {
    match pod_import::engine_loops(&base) {
        Ok(loops) => {
            let [idle, m1, m2] =
                [loops.idle, loops.m1, loops.m2].map(|each| sounds.add(LoopSound(each)));
            commands.insert_resource(LoopHandles([idle, m1, m2]));
        }
        Err(error) => {
            info!("The engines are silent: {error}");
            commands.remove_resource::<LoopHandles>();
        }
    }
}

/// Gives each drawn truck its three loops, silent until `tune_engines` sets them.
pub(super) fn start_engines(
    mut commands: Commands,
    loops: Option<Res<LoopHandles>>,
    visuals: Query<(Entity, &TruckVisual), Without<EngineStarted>>,
) {
    let Some(loops) = loops else {
        return;
    };
    for (visual, drawn) in &visuals {
        commands.entity(visual).insert(EngineStarted);
        for (layer, sound) in loops.0.iter().enumerate() {
            commands.spawn((
                EngineVoice {
                    truck: drawn.truck,
                    layer,
                    share: 0.0,
                },
                AudioPlayer(sound.clone()),
                PlaybackSettings::LOOP
                    .with_volume(Volume::SILENT)
                    .with_spatial(true)
                    .with_spatial_scale(SpatialScale::new(1.0 / FULL_VOLUME_WITHIN)),
                Transform::default(),
                ChildOf(visual),
            ));
        }
    }
}

/// Sets each loop's speed and loudness from its truck's engine.
pub(super) fn tune_engines(
    time: Res<Time<Virtual>>,
    settings: Res<SoundSettings>,
    trucks: Query<(&Engine, &Gearbox, &TruckInput)>,
    mut voices: Query<(&mut EngineVoice, &mut SpatialAudioSink)>,
) {
    for (mut voice, mut sink) in &mut voices {
        let Ok((engine, gearbox, input)) = trucks.get(voice.truck) else {
            continue;
        };
        if time.is_paused() {
            sink.pause();
            continue;
        }
        sink.play();
        let playing = engine.running() && loop_for(gearbox.gear(), engine.revs()) == voice.layer;
        voice.share = fade(voice.share, playing, time.delta_secs());
        sink.set_speed(loop_speed(voice.layer, engine.revs()));
        let gain = voice.share.sqrt() * loudness(input.throttle, engine.running());
        sink.set_volume(Volume::Linear(gain * LOUDNESS * settings.volume));
    }
}

/// Which loop an engine in `gear`, at `revs` of its rev limit, plays: idle in neutral and
/// standing in first or reverse, M1 pulling away in first or reverse, and M2 in every
/// other gear.
fn loop_for(gear: Gear, revs: f32) -> usize {
    match gear {
        Gear::Neutral => IDLE,
        Gear::Reverse | Gear::Forward(1) if revs < IDLE_REVS => IDLE,
        Gear::Reverse | Gear::Forward(1) => M1,
        Gear::Forward(_) => M2,
    }
}

/// How fast to play `layer` for an engine at `revs` of its rev limit, as a share of the
/// speed it was recorded at.
fn loop_speed(layer: usize, revs: f32) -> f32 {
    let (slowest, fastest) = SPEEDS[layer];
    slowest + (fastest - slowest) * revs.clamp(0.0, 1.0)
}

/// How far a loop is faded in after `dt` seconds, from `share`, towards all of it if it
/// is `playing` and none if not, over `FADE_TIME`.
fn fade(share: f32, playing: bool, dt: f32) -> f32 {
    let step = dt / FADE_TIME;
    if playing {
        (share + step).min(1.0)
    } else {
        (share - step).max(0.0)
    }
}

/// How loud a `running` engine is, as a share of full throttle, with `throttle` from -1 to
/// 1 either way: before `LOUDNESS` and the settings. On the throttle, not on the engine's
/// fuel: the loops were recorded on and off the throttle, and that is what they sound like.
fn loudness(throttle: f32, running: bool) -> f32 {
    if running {
        OFF_THROTTLE + (1.0 - OFF_THROTTLE) * throttle.abs().min(1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_and_reverse_pull_away_on_m1_and_every_other_gear_runs_on_m2() {
        assert_eq!(loop_for(Gear::Forward(1), 0.5), M1);
        assert_eq!(loop_for(Gear::Reverse, 0.5), M1);
        for gear in 2..=crate::truck::FORWARD_GEARS {
            assert_eq!(loop_for(Gear::Forward(gear), 0.0), M2);
            assert_eq!(loop_for(Gear::Forward(gear), 1.0), M2);
        }
    }

    #[test]
    fn the_engine_idles_in_neutral_and_standing_in_first_or_reverse() {
        assert_eq!(loop_for(Gear::Neutral, 0.0), IDLE);
        // Revved in neutral, it is still the idle loop, played faster.
        assert_eq!(loop_for(Gear::Neutral, 0.9), IDLE);
        assert_eq!(loop_for(Gear::Forward(1), 0.0), IDLE);
        assert_eq!(loop_for(Gear::Reverse, 0.0), IDLE);
    }

    #[test]
    fn the_pitch_climbs_through_a_gear_and_drops_at_a_change_up() {
        let (slowest, fastest) = SPEEDS[M1];
        assert_eq!(loop_speed(M1, 0.0), slowest);
        assert_eq!(loop_speed(M1, 1.0), fastest);
        assert!(loop_speed(M2, 0.55) < loop_speed(M1, 0.92));
        assert_eq!(loop_speed(IDLE, 0.0), 1.0);
    }

    #[test]
    fn a_change_of_loop_fades_over_the_fade_time() {
        let dt = FADE_TIME / 4.0;
        let mut coming = 0.0;
        let mut going = 1.0;
        for _ in 0..4 {
            coming = fade(coming, true, dt);
            going = fade(going, false, dt);
        }
        assert!((coming - 1.0).abs() < 1e-5 && going.abs() < 1e-5);
        assert_eq!(fade(1.0, true, dt), 1.0);
        assert_eq!(fade(0.0, false, dt), 0.0);
    }

    #[test]
    fn the_throttle_is_louder_and_a_stalled_engine_is_silent() {
        assert_eq!(loudness(0.0, true), OFF_THROTTLE);
        assert_eq!(loudness(-1.0, true), 1.0);
        assert_eq!(loudness(1.0, false), 0.0);
    }

    #[test]
    fn a_loop_plays_over_and_over() {
        let mut decoder = LoopSound(Loop {
            sample_rate: 22050,
            samples: Arc::from([0.1, 0.2, 0.3].as_slice()),
        })
        .decoder();
        let played: Vec<f32> = decoder.by_ref().take(7).collect();
        assert_eq!(played, vec![0.1, 0.2, 0.3, 0.1, 0.2, 0.3, 0.1]);
        assert_eq!(decoder.sample_rate().get(), 22050);
    }
}
