//! Particles moved on the graphics card: the dirt that trucks throw up and the water they
//! splash.
//!
//! A pool of particles is one mesh of small squares, one for each slot, and a storage
//! buffer on the graphics card with one record for each slot. When a particle is thrown,
//! the CPU writes its record once: where it starts, how fast it goes, when it was thrown
//! and when it goes. `particles.wgsl` works out where it is, and turns and sizes it, every
//! frame from those and the time. So the CPU's work goes with how many are thrown, not
//! with how many are in the air: as entities, one each, the particles cost a transform, a
//! cull, an extraction and a sort each, every frame.
//!
//! Only the records thrown in a frame go to the graphics card, straight into their place in
//! the buffer, from the render world (`upload`). The mesh never changes after it is made.
//! When the particles were in the mesh, one particle thrown sent the whole mesh again: with
//! 2000 slots, half a megabyte, nearly every frame while driving.
//!
//! How a particle moves has an exact answer: gravity, and drag towards the moving air,
//! which the shader works out for any age (see `Motion::at`). The air is the same for a
//! whole pool, and gusts (`Air`): the shader works out how it moves now from its clock, so
//! that a gusting wind sends nothing to the graphics card, and a gust moves all the pool's
//! particles a little. When a particle comes down on the ground or the water is worked out
//! on the CPU when it is thrown, by following its path, so that the shader needs no map of
//! the ground. The path is followed without the air, which only moves it across the ground.
//!
//! The squares of one pool are not sorted against each other: the particles are small, soft
//! and see-through, and drawn in the order they are in the mesh. The pool's entity is kept
//! at the camera, so that it is sorted with what is nearest, over the water.
//!
//! The shader's clock (`globals.time`) goes back to 0 every hour, when the particles in the
//! air are lost.
//!
//! Particles are not lit, as lit squares turned to the camera would light oddly. So that
//! they do not shine in the dark, every pool is as bright as `ParticleLight` says, which
//! whoever knows how the world is lit writes (the `weather` slice).
//!
//! Not a slice: a helper for `dirt` and `water`, which each spawn their own pools. Only
//! for an app that can draw.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::system::SystemParam;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::encase::StorageBuffer;
use bevy::render::render_resource::{AsBindGroup, ShaderSize, ShaderType};
use bevy::render::renderer::RenderQueue;
use bevy::render::storage::{GpuShaderBuffer, ShaderBuffer};
use bevy::render::{ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems};
use bevy::shader::ShaderRef;

/// Where `embedded_asset!` puts `particles.wgsl`: the crate's name, then the path below
/// `src`.
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/particles/particles.wgsl";

/// How far apart, in seconds, the points of its path are that a particle is looked for
/// under the ground at when it is thrown. Between the one above and the one under, the
/// place it comes down is then found to within a sixteenth of this.
const PATH_STEP: f32 = 1.0 / 30.0;
const PATH_HALVINGS: usize = 4;

pub type ParticleMaterial = ExtendedMaterial<StandardMaterial, ParticleMotion>;

/// Makes particles work in `app`: their material, and what writes them to their meshes.
/// Both slices that throw particles call it; the second call does nothing.
pub fn add(app: &mut App) {
    if app.is_plugin_added::<MaterialPlugin<ParticleMaterial>>() {
        return;
    }
    embedded_asset!(app, "particles.wgsl");
    app.add_plugins(MaterialPlugin::<ParticleMaterial>::default())
        .init_resource::<Uploads>()
        .init_resource::<ParticleLight>()
        .add_systems(Update, light_particles)
        // After every slice's `Update`, where they are thrown, so that they are drawn in
        // the frame they are thrown in.
        .add_systems(PostUpdate, draw);
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .init_resource::<Uploads>()
            .add_systems(ExtractSchedule, extract_uploads)
            // After the buffers are made, which a pool's first frame may need.
            .add_systems(Render, upload.in_set(RenderSystems::PrepareResources));
    }
}

/// How brightly particles are drawn, from 0 (black) to 1 (their own colours, as in full
/// daylight).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ParticleLight(pub f32);

impl Default for ParticleLight {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Makes every pool as bright as `ParticleLight` says. Only a real change is sent.
fn light_particles(
    light: Res<ParticleLight>,
    pools: Query<&Particles>,
    mut materials: ResMut<Assets<ParticleMaterial>>,
) {
    let level = light.0.clamp(0.0, 1.0);
    let color = Color::linear_rgb(level, level, level);
    for pool in &pools {
        if materials
            .get(&pool.material)
            .is_some_and(|material| material.base.base_color != color)
            && let Some(mut material) = materials.get_mut(&pool.material)
        {
            material.base.base_color = color;
        }
    }
}

/// How the particles of one pool move and look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// How fast they fall, in m/s². Below 0, they rise.
    pub gravity: f32,
    /// How much of their speed through the air they lose each second. Above 0.
    pub drag: f32,
    /// How long one lasts, in seconds, if it doesn't come down first.
    pub life: f32,
    /// Whether one is gone when it comes down on the ground or the water, rather than going
    /// on through it.
    pub lands: bool,
    /// How fast they spread, in metres of radius each second.
    pub spread: f32,
    /// How old one is when it starts to fade out, in seconds: it is clear at `life`. At
    /// `life` or later, it doesn't fade.
    pub fade_from: f32,
    /// How old one is when it starts to shrink away, in seconds: it is gone at `life`. At
    /// `life` or later, it doesn't shrink.
    pub shrink_from: f32,
    /// How long the eye sees one for, in seconds: above 0, it is drawn as a streak along
    /// the way it seems to go, as long as the way it goes in that time. At 0, it is a
    /// square turned to the camera.
    pub exposure: f32,
    /// How near the camera one starts to look smaller the nearer it comes, in metres, so
    /// that none go past the camera as big blobs.
    pub near_camera: f32,
}

impl Motion {
    /// Where a particle thrown from `from` at `velocity`, in air going at `air`, is `age`
    /// seconds later, and how fast it goes then. As `particles.wgsl` works it out.
    ///
    /// The air slows it by `drag` times its speed through the air, and gravity pulls it
    /// down: so it tends to the speed at which the two balance, `settle`, and closes on it
    /// by the same share each second.
    pub fn at(&self, from: Vec3, velocity: Vec3, air: Vec3, age: f32) -> (Vec3, Vec3) {
        let settle = air - Vec3::Y * self.gravity / self.drag;
        let decay = (-self.drag * age).exp();
        let at = from + settle * age + (velocity - settle) * (1.0 - decay) / self.drag;
        (at, settle + (velocity - settle) * decay)
    }

    /// How long a particle thrown from `from` at `velocity` lasts: until it comes down
    /// below `floor` (the height under a point of the ground plane), if it lands, and no
    /// longer than its life.
    fn lifetime(&self, from: Vec3, velocity: Vec3, floor: impl Fn(Vec2) -> f32) -> f32 {
        if !self.lands {
            return self.life;
        }
        let under = |age: f32| {
            let (at, velocity) = self.at(from, velocity, Vec3::ZERO, age);
            velocity.y < 0.0 && at.y < floor(at.xz())
        };
        let mut age = PATH_STEP;
        while age < self.life {
            if under(age) {
                // Back to just under, between the step above and this one.
                let (mut above, mut below) = (age - PATH_STEP, age);
                for _ in 0..PATH_HALVINGS {
                    let middle = (above + below) / 2.0;
                    if under(middle) {
                        below = middle;
                    } else {
                        above = middle;
                    }
                }
                return below;
            }
            age += PATH_STEP;
        }
        self.life
    }
}

/// How the air moves that carries the particles of a pool: steadily, and in gusts on top,
/// which are `gusts` times a mix of three swings of the shader's clock. `particles.wgsl`
/// works it out every frame (`Air::at`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    /// How it moves between gusts, in m/s.
    pub steady: Vec3,
    /// How far a full gust takes it from `steady`, in m/s.
    pub gusts: Vec3,
    /// How long each of the three swings takes, in seconds. Above 0.
    pub periods: [f32; 3],
    /// How much of `gusts` each swing is.
    pub weights: [f32; 3],
}

impl Air {
    /// Air that doesn't move.
    pub const STILL: Air = Air {
        steady: Vec3::ZERO,
        gusts: Vec3::ZERO,
        periods: [1.0; 3],
        weights: [0.0; 3],
    };

    /// How fast it goes, in m/s, when the shader's clock reads `time`. As `air_now` in
    /// `particles.wgsl`.
    pub fn at(&self, time: f32) -> Vec3 {
        let tau = std::f32::consts::TAU;
        let swing: f32 = (0..3)
            .map(|index| self.weights[index] * (tau * time / self.periods[index]).sin())
            .sum();
        self.steady + self.gusts * swing
    }
}

/// One particle, as it is thrown.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    /// Where it starts, in metres.
    pub at: Vec3,
    /// How fast it goes, in m/s.
    pub velocity: Vec3,
    /// Its radius, in metres.
    pub size: f32,
    /// How far it is turned in the picture, in radians, and how fast it turns, in radians a
    /// second. Not for a streak, which lies along the way it goes.
    pub turned: f32,
    pub turning: f32,
    /// Its colour, times the pool's picture, and how solid it is, from 0 to 1.
    pub color: LinearRgba,
}

/// How the particles of a pool move, for `particles.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ParticleMotion {
    #[uniform(100)]
    motion: MotionUniform,
    /// One `Slot` for each square of the pool's mesh.
    #[storage(101, read_only)]
    slots: Handle<ShaderBuffer>,
}

/// One particle as the shader reads it, as `Slot` in `particles.wgsl`: 64 bytes, in the
/// buffer at `slot * Slot::SHADER_SIZE`.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct Slot {
    at: Vec3,
    size: f32,
    velocity: Vec3,
    turned: f32,
    color: Vec4,
    /// When it was thrown and when it goes, on the shader's clock.
    life: Vec2,
    turning: f32,
}

impl Slot {
    fn new(particle: &Particle, life: [f32; 2]) -> Self {
        Self {
            at: particle.at,
            size: particle.size,
            velocity: particle.velocity,
            turned: particle.turned,
            color: particle.color.to_vec4(),
            life: Vec2::from(life),
            turning: particle.turning,
        }
    }
}

/// Writes of slots to a pool's buffer: in the main world, those of this frame, which the
/// render world takes at extraction and writes to the graphics card.
#[derive(Resource, Default)]
struct Uploads(Vec<Upload>);

/// A run of neighbouring slots, written as one.
struct Upload {
    buffer: AssetId<ShaderBuffer>,
    /// Where the first slot starts in the buffer, in bytes.
    offset: u64,
    bytes: Vec<u8>,
}

impl MaterialExtension for ParticleMotion {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }
}

/// As `Motion` in `particles.wgsl`, where each is explained. Each `Vec3` is followed by
/// an `f32`, which fills out its 16 bytes.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct MotionUniform {
    air: Vec3,
    gravity: f32,
    gusts: Vec3,
    drag: f32,
    gust_periods: Vec3,
    life: f32,
    gust_weights: Vec3,
    spread: f32,
    fade_from: f32,
    shrink_from: f32,
    exposure: f32,
    near_camera: f32,
}

impl MotionUniform {
    fn new(motion: Motion, air: Air) -> Self {
        Self {
            air: air.steady,
            gravity: motion.gravity,
            gusts: air.gusts,
            drag: motion.drag,
            gust_periods: Vec3::from(air.periods),
            life: motion.life,
            gust_weights: Vec3::from(air.weights),
            spread: motion.spread,
            fade_from: motion.fade_from,
            shrink_from: motion.shrink_from,
            exposure: motion.exposure,
            near_camera: motion.near_camera,
        }
    }
}

/// A pool of particles, on the entity that draws them.
#[derive(Component)]
pub struct Particles {
    motion: Motion,
    /// Per slot: when its particle was thrown and when it goes, on the shader's clock.
    lives: Vec<[f32; 2]>,
    /// Where to start looking for a free slot.
    next: usize,
    /// The particles thrown since they were last sent to the buffer, and their slots.
    thrown: Vec<(usize, Particle, [f32; 2])>,
    buffer: Handle<ShaderBuffer>,
    material: Handle<ParticleMaterial>,
}

impl Particles {
    /// How many more particles could be thrown now. `now` is `clock(time)`.
    pub fn room(&self, now: f32) -> usize {
        self.lives
            .iter()
            .filter(|life| is_free(**life, now))
            .count()
    }

    /// Throws `particle` now (`clock(time)`), if there is room. It comes down where it goes
    /// below `floor`, the height under a point of the ground plane, if its pool's
    /// particles land. Returns whether it was thrown.
    pub fn throw(&mut self, now: f32, particle: Particle, floor: impl Fn(Vec2) -> f32) -> bool {
        let slots = self.lives.len();
        let Some(slot) = (0..slots)
            .map(|step| (self.next + step) % slots)
            .find(|slot| is_free(self.lives[*slot], now))
        else {
            return false;
        };
        self.next = (slot + 1) % slots;
        let lasts = self.motion.lifetime(particle.at, particle.velocity, floor);
        let life = [now, now + lasts];
        self.lives[slot] = life;
        self.thrown.push((slot, particle, life));
        true
    }
}

/// The time on the shader's clock, in seconds, for `Particles::throw` and `Particles::room`.
pub fn clock(time: &Time) -> f32 {
    time.elapsed_secs_wrapped()
}

/// Whether a slot whose particle was thrown and goes at `life` is free `now`. After the
/// clock goes back to 0, one thrown before is gone.
fn is_free([thrown, goes]: [f32; 2], now: f32) -> bool {
    now >= goes || now < thrown
}

/// Where a pool's mesh, material and buffer are kept: what `pool` needs.
#[derive(SystemParam)]
pub struct PoolAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ParticleMaterial>>,
    buffers: ResMut<'w, Assets<ShaderBuffer>>,
}

/// A pool of `slots` particles that move as `motion` says, carried by `air`, drawn with
/// `picture`: a picture in white, with its top at the front of a streak. Spawn it with what
/// else the slice needs on it.
pub fn pool(
    assets: &mut PoolAssets,
    slots: usize,
    motion: Motion,
    air: Air,
    picture: Handle<Image>,
) -> impl Bundle {
    let mesh = assets.meshes.add(squares(slots));
    // Made on the graphics card, where it starts as zeros: every slot empty. Nothing is
    // kept on the CPU; `upload` writes the slots as they are thrown.
    let buffer = assets.buffers.add(ShaderBuffer::with_size(
        slots * Slot::SHADER_SIZE.get() as usize,
        RenderAssetUsages::default(),
    ));
    let material = assets.materials.add(ParticleMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(picture),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            // Seen from either side: a streak turns its face to the camera only about its
            // length.
            cull_mode: None,
            ..default()
        },
        extension: ParticleMotion {
            motion: MotionUniform::new(motion, air),
            slots: buffer.clone(),
        },
    });
    (
        Particles {
            motion,
            lives: vec![[0.0, 0.0]; slots],
            next: 0,
            thrown: Vec::new(),
            buffer,
            material: material.clone(),
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::default(),
        // Placed by the shader, so never outside the view as a whole.
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    )
}

/// A mesh of `slots` squares. Its first UV is which corner of its square a corner is, the
/// top of the picture at the first two, and its second which slot the square is, which
/// the shader reads the particle from. It never changes, so it is only on the graphics
/// card. It has colours, all zero, only so that the standard material takes the colour the
/// shader gives each corner.
fn squares(slots: usize) -> Mesh {
    let corners: Vec<[f32; 2]> = (0..slots)
        .flat_map(|_| [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        .collect();
    // Exact as a float up to 2^24 slots.
    let slot_of_corner: Vec<[f32; 2]> = (0..slots)
        .flat_map(|slot| [[slot as f32, 0.0]; 4])
        .collect();
    let indices: Vec<u32> = (0..slots as u32)
        .flat_map(|square| {
            let first = square * 4;
            [first, first + 2, first + 1, first, first + 3, first + 2]
        })
        .collect();
    let vertices = slots * 4;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; vertices])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, slot_of_corner)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; vertices])
    .with_inserted_indices(Indices::U32(indices))
}

/// Hands the particles thrown this frame to the render world, and keeps each pool at the
/// camera.
fn draw(
    mut uploads: ResMut<Uploads>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut pools: Query<(&mut Particles, &mut Transform)>,
) {
    let eye = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map(|(_, transform)| transform.translation());
    for (mut pool, mut transform) in &mut pools {
        if let Some(eye) = eye {
            transform.translation = eye;
        }
        if pool.thrown.is_empty() {
            continue;
        }
        let thrown = std::mem::take(&mut pool.thrown);
        let buffer = pool.buffer.id();
        uploads
            .0
            .extend(runs(&thrown).into_iter().map(|(offset, bytes)| Upload {
                buffer,
                offset,
                bytes,
            }));
    }
}

/// `thrown` as runs of neighbouring slots, each as where it starts in the buffer, in
/// bytes, and its records. Slots are taken in turn, so what is thrown in one frame is
/// mostly one run.
fn runs(thrown: &[(usize, Particle, [f32; 2])]) -> Vec<(u64, Vec<u8>)> {
    let mut sorted: Vec<_> = thrown.iter().collect();
    // Stable, so that of a slot written twice in one frame, the later one wins.
    sorted.sort_by_key(|(slot, ..)| *slot);
    let size = Slot::SHADER_SIZE.get();
    let mut runs: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut next_slot = None;
    for (slot, particle, life) in sorted {
        let mut bytes = StorageBuffer::new(Vec::with_capacity(size as usize));
        bytes
            .write(&Slot::new(particle, *life))
            .expect("a slot fits in its own bytes");
        let bytes = bytes.into_inner();
        match runs.last_mut() {
            // The same slot again: the later particle replaces the earlier.
            Some((_, run)) if next_slot == Some(*slot + 1) => {
                let at = run.len() - size as usize;
                run[at..].copy_from_slice(&bytes);
            }
            Some((_, run)) if next_slot == Some(*slot) => run.extend_from_slice(&bytes),
            _ => runs.push((*slot as u64 * size, bytes)),
        }
        next_slot = Some(*slot + 1);
    }
    runs
}

/// Takes this frame's uploads from the main world.
fn extract_uploads(mut main_world: ResMut<MainWorld>, mut uploads: ResMut<Uploads>) {
    if let Some(mut thrown) = main_world.get_resource_mut::<Uploads>() {
        uploads.0.append(&mut thrown.0);
    }
}

/// Writes this frame's slots into their pools' buffers. A pool's buffer is made in the
/// frame the pool is, before this; one that is gone takes nothing.
fn upload(
    mut uploads: ResMut<Uploads>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    queue: Res<RenderQueue>,
) {
    for upload in uploads.0.drain(..) {
        if let Some(buffer) = buffers.get(upload.buffer) {
            queue.write_buffer(&buffer.buffer, upload.offset, &upload.bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THROWN: Motion = Motion {
        gravity: 9.81,
        drag: 0.6,
        life: 1.6,
        lands: true,
        spread: 0.0,
        fade_from: 1.6,
        shrink_from: 1.0,
        exposure: 0.04,
        near_camera: 6.0,
    };

    fn pool(slots: usize, motion: Motion) -> Particles {
        Particles {
            motion,
            lives: vec![[0.0, 0.0]; slots],
            next: 0,
            thrown: Vec::new(),
            buffer: Handle::default(),
            material: Handle::default(),
        }
    }

    fn particle(velocity: Vec3) -> Particle {
        Particle {
            at: Vec3::ZERO,
            velocity,
            size: 0.1,
            turned: 0.0,
            turning: 0.0,
            color: LinearRgba::WHITE,
        }
    }

    #[test]
    fn a_particle_moves_as_small_steps_of_gravity_and_drag_would_move_it() {
        let air = Vec3::new(4.0, 0.0, -3.0);
        let (from, velocity) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(-2.0, 6.0, 1.0));
        for motion in [
            THROWN,
            Motion {
                gravity: -0.3,
                drag: 2.0,
                ..THROWN
            },
        ] {
            let (mut at, mut moving) = (from, velocity);
            let dt = 1e-4;
            for step in 1..=10_000 {
                moving += (Vec3::NEG_Y * motion.gravity - (moving - air) * motion.drag) * dt;
                at += moving * dt;
                if step % 2500 == 0 {
                    let (exact_at, exact_moving) = motion.at(from, velocity, air, step as f32 * dt);
                    assert!(at.distance(exact_at) < 2e-3, "{at} {exact_at}");
                    assert!(
                        moving.distance(exact_moving) < 2e-3,
                        "{moving} {exact_moving}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_particle_that_lands_lasts_until_it_comes_down() {
        let up = particle(Vec3::Y * 4.0);
        let lasts = THROWN.lifetime(up.at, up.velocity, |_| 0.0);
        // Back down, and no later than the path's steps find it there.
        let (at, moving) = THROWN.at(up.at, up.velocity, Vec3::ZERO, lasts);
        assert!(at.y < 0.0 && moving.y < 0.0);
        let (before, _) = THROWN.at(up.at, up.velocity, Vec3::ZERO, lasts - PATH_STEP / 16.0);
        assert!(before.y >= 0.0, "{before}");
        // Over ground it never comes down to, it lasts its life; and so does one that
        // doesn't land.
        assert_eq!(THROWN.lifetime(up.at, up.velocity, |_| -100.0), THROWN.life);
        let floating = Motion {
            lands: false,
            ..THROWN
        };
        assert_eq!(floating.lifetime(up.at, up.velocity, |_| 0.0), THROWN.life);
    }

    #[test]
    fn a_particle_needs_a_free_slot() {
        let mut pool = pool(3, THROWN);
        let never_lands = |_: Vec2| -100.0;
        assert_eq!(pool.room(10.0), 3);
        for _ in 0..3 {
            assert!(pool.throw(10.0, particle(Vec3::Y), never_lands));
        }
        assert_eq!(pool.room(10.0), 0);
        assert!(!pool.throw(10.5, particle(Vec3::Y), never_lands));
        // Its slots come free as they go.
        assert_eq!(pool.room(10.0 + THROWN.life), 3);
        // And when the clock goes back to 0.
        assert_eq!(pool.room(0.2), 3);
        // Each is sent to the buffer once, in its own slot.
        let mut slots: Vec<usize> = pool.thrown.iter().map(|(slot, ..)| *slot).collect();
        slots.sort();
        assert_eq!(slots, [0, 1, 2]);
    }

    #[test]
    fn neighbouring_slots_are_sent_as_one_run_at_their_place_in_the_buffer() {
        let size = Slot::SHADER_SIZE.get();
        let thrown = |slot: usize, goes: f32| (slot, particle(Vec3::Y), [1.0, goes]);
        let runs = runs(&[
            thrown(5, 2.0),
            thrown(3, 2.0),
            thrown(4, 2.0),
            thrown(9, 2.0),
        ]);
        let placed: Vec<(u64, usize)> = runs.iter().map(|(at, bytes)| (*at, bytes.len())).collect();
        assert_eq!(
            placed,
            [(3 * size, 3 * size as usize), (9 * size, size as usize)]
        );
    }

    #[test]
    fn a_slot_thrown_twice_in_one_frame_keeps_the_later_particle() {
        let first = (2, particle(Vec3::Y), [1.0, 2.0]);
        let second = (2, particle(Vec3::Y), [1.5, 3.0]);
        let runs = runs(&[first, second]);
        assert_eq!(runs.len(), 1);
        let mut expected = StorageBuffer::new(Vec::<u8>::new());
        expected.write(&Slot::new(&second.1, second.2)).unwrap();
        assert_eq!(runs[0].1, expected.into_inner());
    }

    #[test]
    fn the_shader_reads_a_slot_in_the_order_it_is_written() {
        // WGSL lays out a struct as encase does, so the fields in the same order give the
        // same 64 bytes.
        assert_eq!(Slot::SHADER_SIZE.get(), 64);
        let shader = include_str!("particles.wgsl");
        let fields = [
            "at", "size", "velocity", "turned", "color", "life", "turning",
        ];
        let mut from = shader.find("struct Slot {").expect("struct Slot");
        for field in fields {
            let at = shader[from..]
                .find(&format!("    {field}: "))
                .unwrap_or_else(|| panic!("{field} out of order in particles.wgsl"));
            from += at + 1;
        }
    }

    #[test]
    fn the_shader_reads_the_motion_in_the_order_it_is_written() {
        let shader = include_str!("particles.wgsl");
        let fields = [
            "air",
            "gravity",
            "gusts",
            "drag",
            "gust_periods",
            "life",
            "gust_weights",
            "spread",
            "fade_from",
            "shrink_from",
            "exposure",
            "near_camera",
        ];
        let mut from = shader.find("struct Motion {").expect("struct Motion");
        for field in fields {
            let at = shader[from..]
                .find(&format!("    {field}: "))
                .unwrap_or_else(|| panic!("{field} out of order in particles.wgsl"));
            from += at + 1;
        }
        let air = Air {
            steady: Vec3::X,
            gusts: Vec3::Z,
            periods: [3.0, 2.0, 1.0],
            weights: [0.5, 0.3, 0.2],
        };
        let uniform = MotionUniform::new(THROWN, air);
        assert_eq!(uniform.air, Vec3::X);
        assert_eq!(uniform.gusts, Vec3::Z);
        assert_eq!(uniform.gust_periods, Vec3::new(3.0, 2.0, 1.0));
        assert_eq!(uniform.gust_weights, Vec3::new(0.5, 0.3, 0.2));
        assert_eq!(uniform.near_camera, THROWN.near_camera);
    }

    #[test]
    fn the_air_gusts_about_its_steady_speed() {
        let air = Air {
            steady: Vec3::new(3.0, 0.0, 0.0),
            gusts: Vec3::new(1.0, 0.0, 0.0),
            periods: [4.0, 2.0, 1.0],
            weights: [0.5, 0.3, 0.2],
        };
        // Every swing is at 0 at 0 and at a whole period.
        assert!(air.at(0.0).distance(air.steady) < 1e-6);
        assert!(air.at(4.0).distance(air.steady) < 1e-5);
        // A quarter of the way through the slow swing, at its height, the others at 0 or
        // back to it.
        assert!(air.at(1.0).distance(Vec3::new(3.5, 0.0, 0.0)) < 1e-5);
        assert_eq!(Air::STILL.at(12.3), Vec3::ZERO);
    }

    #[test]
    fn the_shader_gusts_as_the_air_does() {
        let shader = include_str!("particles.wgsl");
        assert!(shader.contains(
            "motion.air + motion.gusts * dot(motion.gust_weights, sin(TAU * time / motion.gust_periods))"
        ));
    }
}
