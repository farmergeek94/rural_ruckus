//! A truck to look at: its body, tires and axles at their rest positions, with nothing of
//! the physics. This is the one way a truck is drawn. The truck that is driven is built by
//! the same function (see `spawn`), so a truck in a showroom cannot drift from the truck in
//! the race.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

use super::axle::{Axle, LinkToAxle};
use super::config::FRONT_WHEELS;
use super::looks::{Models, Paintwork, Part};
use super::{TruckConfig, TruckData, TruckLooksSettings, Wheel};

/// Put this on an entity and the truck is built as its children, standing as it does at
/// rest: the chassis centre at the entity's origin and the tires
/// `TruckConfig::standing_height` below it. No body, collider or `Truck` marker.
#[derive(Component)]
pub struct TruckDisplay(pub TruckData);

/// The built-in truck's body, which is also the collider of a truck that gives no shape.
pub(super) const CHASSIS_HALF_EXTENTS: Vec3 = Vec3::new(1.1, 0.45, 2.3);

/// The asset stores a truck is drawn with. All absent in a headless app, where a truck is
/// built as bare pivots, and the images alone where there is nothing to texture with.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Paint<'w> {
    meshes: Option<ResMut<'w, Assets<Mesh>>>,
    materials: Option<ResMut<'w, Assets<StandardMaterial>>>,
    images: Option<ResMut<'w, Assets<Image>>>,
    settings: Res<'w, TruckLooksSettings>,
}

pub(super) fn build_displays(
    mut commands: Commands,
    displays: Query<(Entity, &TruckDisplay), Added<TruckDisplay>>,
    mut paint: Paint,
) {
    for (entity, display) in &displays {
        commands.entity(entity).insert_if_new(Visibility::default());
        build(
            &mut commands,
            entity,
            &display.0,
            &display.0.config,
            &mut paint,
        );
    }
}

/// Builds the truck under `parent`, and returns its wheels in `TruckConfig::wheel_rest`'s
/// order. `config` is the truck's own, or that with a setup laid over it. Whatever the truck doesn't supply is the built-in truck's: a box for a body,
/// cylinders for tires, and no axles.
pub(super) fn build(
    commands: &mut Commands,
    parent: Entity,
    data: &TruckData,
    config: &TruckConfig,
    paint: &mut Paint,
) -> Vec<Entity> {
    let mut shapes = match (&mut paint.meshes, &mut paint.materials) {
        (Some(meshes), Some(materials)) => Some((meshes, materials)),
        _ => None,
    };
    let models = match (&data.looks, &mut shapes, &mut paint.images) {
        (Some(looks), Some((meshes, materials)), Some(images)) => {
            Models::build(looks, meshes, materials, images, &paint.settings)
        }
        _ => Models::default(),
    };
    // Despawned with the truck, as its parts are.
    for cycle in &models.cycles {
        commands.spawn((cycle.clone(), ChildOf(parent)));
    }

    let mut builtin_body = Vec::new();
    let mut builtin_tire = Vec::new();
    if let Some((meshes, materials)) = &mut shapes {
        if models.body.is_none() {
            builtin_body.push((
                meshes.add(Cuboid::from_size(CHASSIS_HALF_EXTENTS * 2.0)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb_u8(
                        data.body_color[0],
                        data.body_color[1],
                        data.body_color[2],
                    ),
                    reflectance: paint.settings.reflectance,
                    ..default()
                }),
            ));
        }
        if models.left_tire.is_none() || models.right_tire.is_none() {
            builtin_tire.push((
                meshes.add(Cylinder::new(config.wheel_radius, config.wheel_width)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.03, 0.03, 0.03),
                    perceptual_roughness: 1.0,
                    reflectance: paint.settings.reflectance,
                    ..default()
                }),
            ));
        }
    }

    let body = models.body.unwrap_or(builtin_body);
    spawn_parts(commands, parent, &body, Quat::IDENTITY);

    // Bevy cylinders stand along Y, and a wheel's axle lies along X.
    let tire = |model: Option<Vec<Part>>| match model {
        Some(parts) => (parts, Quat::IDENTITY),
        None => (builtin_tire.clone(), Quat::from_rotation_z(FRAC_PI_2)),
    };
    let (left_tire, right_tire) = (tire(models.left_tire), tire(models.right_tire));

    let mut wheel_entities = Vec::new();
    for (index, (&rest, mount)) in config
        .wheel_rest
        .iter()
        .zip(config.wheel_mounts())
        .enumerate()
    {
        // A pivot that steers and spins (see `drive`), which the tire hangs from.
        let wheel = commands
            .spawn((
                Wheel {
                    mount,
                    front: index < FRONT_WHEELS,
                    spin: 0.0,
                    tread_speed: 0.0,
                    tilt: 0.0,
                },
                Transform::from_translation(rest),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        let (side, place) = if rest.x < 0.0 {
            (&left_tire, 0)
        } else {
            (&right_tire, 1)
        };
        let axle = if index < FRONT_WHEELS { 0 } else { 2 };
        let (parts, rotation) = match &models.wheel_tires[axle + place] {
            Some(parts) => (parts, Quat::IDENTITY),
            None => (&side.0, side.1),
        };
        spawn_parts(commands, wheel, parts, rotation);
        wheel_entities.push(wheel);
    }

    if let Some(axle) = &models.axle {
        for number in 0..wheel_entities.len() / 2 {
            let (left, right) = axle_hubs(config, &wheel_entities, number);
            let entity = commands
                .spawn((
                    Axle { left, right },
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(parent),
                ))
                .id();
            spawn_parts(commands, entity, axle, Quat::IDENTITY);
        }
    }

    for ((mesh, material), links) in &models.axle_links {
        for link in links
            .iter()
            .filter(|link| link.axle < wheel_entities.len() / 2)
        {
            let (left, right) = axle_hubs(config, &wheel_entities, link.axle);
            commands.spawn((
                LinkToAxle {
                    left,
                    right,
                    body_end: link.body_end,
                    axle_end: link.axle_end,
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Paintwork,
                Transform::default(),
                ChildOf(parent),
            ));
        }
    }

    wheel_entities
}

/// The left and right hubs of an axle, counting from the front. Wheels come in pairs, an
/// axle at a time, either side first.
fn axle_hubs(config: &TruckConfig, wheels: &[Entity], axle: usize) -> (Entity, Entity) {
    let first = 2 * axle;
    let (left, right) = (wheels[first], wheels[first + 1]);
    if config.wheel_rest[first].x > 0.0 {
        (right, left)
    } else {
        (left, right)
    }
}

fn spawn_parts(commands: &mut Commands, parent: Entity, parts: &[Part], rotation: Quat) {
    for (mesh, material) in parts {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Paintwork,
            Transform::from_rotation(rotation),
            ChildOf(parent),
        ));
    }
}
