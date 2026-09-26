//! What a checkpoint looks like: two solid poles with a banner between them. The
//! banner of the gate to drive through next is lit up.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::Racer;
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::PlayerTruck;

const POLE_RADIUS: f32 = 0.3;
/// Height of the pole tops above the higher of the two pole bases, in metres.
const POLE_HEIGHT: f32 = 7.0;
/// Banner size top to bottom, in metres. It hangs from the pole tops, so its lower edge
/// is well above a truck on the ground.
const BANNER_DEPTH: f32 = 1.5;
const BANNER_THICKNESS: f32 = 0.15;

/// The banner of gate number `0`.
#[derive(Component)]
pub(super) struct GateBanner(usize);

#[derive(Resource)]
pub(super) struct BannerMaterials {
    next: Handle<StandardMaterial>,
    other: Handle<StandardMaterial>,
}

pub(super) fn spawn_gate_markers(
    mut commands: Commands,
    track: Res<Track>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pole_material = materials.add(Color::srgb(0.85, 0.85, 0.85));
    let banner_materials = BannerMaterials {
        next: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.75, 0.0),
            emissive: LinearRgba::rgb(1.5, 1.0, 0.0),
            ..default()
        }),
        other: materials.add(Color::srgb(0.25, 0.3, 0.6)),
    };

    // A track with scenery of its own marks its own checkpoints, with banners and the like.
    let gates = if track.scenery.objects.is_empty() {
        track.gates.as_slice()
    } else {
        &[]
    };
    for (index, gate) in gates.iter().enumerate() {
        // The poles stand just outside the gate, leaving its whole width to drive through.
        let reach = gate.across() * (gate.half_width + POLE_RADIUS);
        let bases = [gate.center - reach, gate.center + reach]
            .map(|base| Vec3::new(base.x, track.heights.height_at(base.x, base.y), base.y));
        let top = bases[0].y.max(bases[1].y) + POLE_HEIGHT;

        for base in bases {
            let height = top - base.y;
            commands.spawn((
                Name::new(format!("Gate {index} pole")),
                DespawnOnExit(GameState::Racing),
                Mesh3d(meshes.add(Cylinder::new(POLE_RADIUS, height))),
                MeshMaterial3d(pole_material.clone()),
                Transform::from_translation(base.with_y(base.y + height / 2.0)),
                RigidBody::Fixed,
                Collider::cylinder(height / 2.0, POLE_RADIUS),
            ));
        }

        let span = bases[0].xz().distance(bases[1].xz());
        let middle = bases[0].midpoint(bases[1]).with_y(top - BANNER_DEPTH / 2.0);
        commands.spawn((
            Name::new(format!("Gate {index} banner")),
            GateBanner(index),
            DespawnOnExit(GameState::Racing),
            Mesh3d(meshes.add(Cuboid::new(span, BANNER_DEPTH, BANNER_THICKNESS))),
            MeshMaterial3d(banner_materials.other.clone()),
            Transform::from_translation(middle).with_rotation(Quat::from_rotation_y(gate.yaw)),
        ));
    }

    commands.insert_resource(banner_materials);
}

/// Follows the player.
pub(super) fn highlight_next_gate(
    racer: Single<&Racer, With<PlayerTruck>>,
    materials: Res<BannerMaterials>,
    mut banners: Query<(&GateBanner, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let racing = racer.progress.finished.is_none();
    for (banner, mut material) in &mut banners {
        let wanted = if racing && banner.0 == racer.progress.next_gate {
            &materials.next
        } else {
            &materials.other
        };
        // Compared first so that the material only counts as changed when it has.
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}
