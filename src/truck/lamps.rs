//! A truck's lamps (`TruckData::lamps`): a glow at each lamp, and a beam from those that
//! have one, which light up when it is dark.
//!
//! Each lamp is an entity on the drawn truck (`TruckVisual`), placed at the lamp and turned
//! the way it shines, so that it moves as smoothly as the truck is drawn. Its glow is a
//! disc facing the way it shines, with the lamp's own picture added onto what is behind
//! it: seen from in front it shines, and from the side it is edge on, as a lamp is. Its
//! beam is a spot light.
//!
//! In MTM2 the beam was a cone drawn with a texture (see `docs/formats/truck.md`). Here it
//! is both: real light, as long and as wide as that cone, which lights the ground and what
//! stands on it, and the cone, drawn added onto what is behind it and fading to nothing at
//! its far end, so that the beam is seen in the air.
//!
//! Whether they shine is `TruckLamps::lit`, which this slice leaves off: whoever knows how
//! dark it is turns them on. A beacon turns round the truck's up, and a blinking lamp goes
//! on and off, by the game's clock.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::looks::texture_image;
use super::{Beam, SeenFromInside, TruckLamp, TruckTexture, TruckVisual};

/// How bright a beam is at its middle, in lumens. Enough to light the course a beam's
/// length ahead at night; higher washes the ground near the truck out to white. At 400 000
/// the beams were lost under the moonlight.
const BEAM_POWER: f32 = 6_000_000.0;
/// How bright the cone of a beam is at the lamp, added onto what is behind it, from 0
/// (not seen) up. Higher is a thicker, mistier beam.
const CONE_BRIGHTNESS: f32 = 0.01;
/// How bright a lamp's glow is, as a multiple of its picture. The race camera draws in high
/// dynamic range with bloom, so above 1 the glow spills light round it; higher shines more.
const GLOW_BRIGHTNESS: f32 = 12.0;
/// The least a beam's light spreads, from its middle to its edge, in radians, and the
/// least far it reaches, in metres. MTM2's headlight cones spread 0.14 and reach 23 m,
/// which, as light, lit a narrow strip a few truck lengths long. The drawn cone keeps
/// MTM2's shape. Wider and longer lights more of the course.
const LEAST_SPREAD: f32 = 0.45;
const LEAST_REACH: f32 = 60.0;
/// How far ahead of the lamp its light starts, in metres, so that it is outside the body.
const AHEAD_OF_LAMP: f32 = 0.3;
/// How many sides the cone of a beam has.
const CONE_SIDES: u32 = 16;
/// How much of a beam's spread is at full brightness, from 0 to 1, before it fades to
/// nothing at its edge. Lower is a softer edge.
const BEAM_CORE: f32 = 0.01;

/// Whether the trucks' lamps shine. Off unless someone turns them on.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct TruckLamps {
    pub lit: bool,
}

/// On each of a truck's lamps, a child of the drawn truck.
#[derive(Component)]
pub(super) struct Lamp {
    lamp: TruckLamp,
    /// The glow's picture, from the truck's textures.
    picture: Option<TruckTexture>,
}

/// Puts the lamps of a truck with `lamps` and `textures` on `visual`, the drawn truck, dark.
pub(super) fn spawn(
    commands: &mut Commands,
    visual: Entity,
    lamps: &[TruckLamp],
    textures: &[TruckTexture],
) {
    for lamp in lamps {
        commands.spawn((
            Name::new("Lamp"),
            Lamp {
                lamp: lamp.clone(),
                picture: lamp.glow.and_then(|index| textures.get(index)).cloned(),
            },
            placed(lamp, 0.0),
            Visibility::Hidden,
            ChildOf(visual),
        ));
    }
}

/// Where a lamp is on its truck, turned the way it shines, and as far round as a beacon has
/// turned `seconds` into the race.
fn placed(lamp: &TruckLamp, seconds: f32) -> Transform {
    let facing = Quat::from_rotation_y(lamp.spin * seconds) * lamp.facing;
    // Straight up or down, any way round will do.
    let up = if facing.cross(Vec3::Y).length_squared() < 1e-6 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Transform::from_translation(lamp.position).looking_to(facing, up)
}

/// Whether a lamp that blinks `blink` is on `seconds` into the race.
fn blinked_on(blink: Option<[f32; 2]>, seconds: f32) -> bool {
    match blink {
        Some([on, off]) => seconds.rem_euclid(on + off) < on,
        None => true,
    }
}

/// Gives each new lamp its glow and its beam. The glow only in an app that can draw.
pub(super) fn dress_lamps(
    mut commands: Commands,
    lamps: Query<(Entity, &Lamp), Added<Lamp>>,
    // All absent in a headless app, which has no use for looks.
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
) {
    for (entity, Lamp { lamp, picture }) in &lamps {
        let [red, green, blue] = lamp.color;
        let color = Color::linear_rgb(red, green, blue);
        if let Some(beam) = lamp.beam {
            if let (Some(meshes), Some(materials)) = (meshes.as_mut(), materials.as_mut()) {
                commands.spawn((
                    BeamCone,
                    Mesh3d(meshes.add(cone_mesh(beam))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::linear_rgb(
                            red * CONE_BRIGHTNESS,
                            green * CONE_BRIGHTNESS,
                            blue * CONE_BRIGHTNESS,
                        ),
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        // Seen from inside as well, where both sides add up.
                        cull_mode: None,
                        ..default()
                    })),
                    NotShadowCaster,
                    ChildOf(entity),
                ));
            }
            let spread = beam.spread.max(LEAST_SPREAD);
            commands.spawn((
                SpotLight {
                    color,
                    intensity: BEAM_POWER,
                    range: beam.reach.max(LEAST_REACH),
                    outer_angle: spread,
                    inner_angle: spread * BEAM_CORE,
                    // The lamp is in the truck's own body, which would shadow its beam.
                    shadow_maps_enabled: false,
                    ..default()
                },
                // Clear of the body round the lamp.
                Transform::from_xyz(0.0, 0.0, -AHEAD_OF_LAMP),
                ChildOf(entity),
            ));
        }
        // A lamp with no glow is its light alone.
        let (Some(meshes), Some(materials), Some(images)) =
            (meshes.as_mut(), materials.as_mut(), images.as_mut())
        else {
            continue;
        };
        if lamp.glow_radius <= 0.0 {
            continue;
        }
        // A lamp with no picture gets a soft round one, in its own colour.
        let texture = Some(images.add(match picture {
            Some(picture) => texture_image(picture),
            None => soft_glow(),
        }));
        commands.spawn((
            Glow,
            Mesh3d(meshes.add(Circle::new(lamp.glow_radius.max(LEAST_GLOW)))),
            MeshMaterial3d(materials.add(StandardMaterial {
                // A picture shows its own colours; the soft glow is white, in the light's.
                base_color: if picture.is_some() {
                    Color::linear_rgb(GLOW_BRIGHTNESS, GLOW_BRIGHTNESS, GLOW_BRIGHTNESS)
                } else {
                    Color::linear_rgb(
                        red * GLOW_BRIGHTNESS,
                        green * GLOW_BRIGHTNESS,
                        blue * GLOW_BRIGHTNESS,
                    )
                },
                base_color_texture: texture,
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            })),
            // Turned to the camera by `turn_glows`.
            Transform::default(),
            NotShadowCaster,
            ChildOf(entity),
        ));
    }
}

/// The cone of `beam`, shining along -Z from the origin: as wide as the beam at each end,
/// white at the lamp and black, which adds nothing, at its far end.
fn cone_mesh(beam: Beam) -> Mesh {
    let far_width = beam.width + beam.reach * beam.spread.tan();
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for side in 0..=CONE_SIDES {
        let round = side as f32 / CONE_SIDES as f32 * std::f32::consts::TAU;
        let (sin, cos) = round.sin_cos();
        positions.push([cos * beam.width, sin * beam.width, 0.0]);
        colors.push([1.0, 1.0, 1.0, 1.0]);
        positions.push([cos * far_width, sin * far_width, -beam.reach]);
        colors.push([0.0, 0.0, 0.0, 1.0]);
    }
    let mut indices = Vec::new();
    for side in 0..CONE_SIDES {
        let near = side * 2;
        indices.extend_from_slice(&[near, near + 2, near + 1, near + 1, near + 2, near + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// On the drawn cone of a beam.
#[derive(Component)]
pub(super) struct BeamCone;

/// Hides the body and wheels of a truck seen from inside (`SeenFromInside`), and the cones
/// of its beams, which would haze the view from the cab, and keeps its lamps lit. Shows
/// them again when the camera comes out.
pub(super) fn show_from_inside(
    visuals: Query<(&Children, Has<SeenFromInside>), With<TruckVisual>>,
    lamps: Query<&Children, With<Lamp>>,
    mut parts: Query<&mut Visibility, (Without<Lamp>, Without<BeamCone>)>,
    mut cones: Query<&mut Visibility, With<BeamCone>>,
) {
    for (children, inside) in &visuals {
        let shown = if inside {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for &child in children {
            if let Ok(lamp_parts) = lamps.get(child) {
                for &part in lamp_parts {
                    if let Ok(mut cone) = cones.get_mut(part) {
                        cone.set_if_neq(shown);
                    }
                }
            } else if let Ok(mut part) = parts.get_mut(child) {
                part.set_if_neq(shown);
            }
        }
    }
}

/// On a lamp's glow, which is turned to face the camera, as a lamp's picture is drawn.
#[derive(Component)]
pub(super) struct Glow;

/// The least a glow is drawn, from its middle to its edge, in metres. Some trucks give
/// theirs a few centimetres, which in the dark is a speck.
const LEAST_GLOW: f32 = 0.15;
/// How big a glow is seen from the side, as a share of how big it is seen from in front.
/// From behind the lamp it is not seen at all.
const GLOW_FROM_THE_SIDE: f32 = 0.35;
/// Pixels along each side of the soft glow of a lamp with no picture.
const SOFT_GLOW_SIZE: u32 = 32;

/// How big a glow is, as a share of its size, seen from a way that is `towards` of the way
/// the lamp shines: 1 straight in front, 0 at the side and behind.
fn glow_scale(towards: f32) -> f32 {
    if towards <= 0.0 {
        0.0
    } else {
        GLOW_FROM_THE_SIDE + (1.0 - GLOW_FROM_THE_SIDE) * towards
    }
}

/// Turns every glow to face the camera, and shrinks it as the lamp turns away. By where
/// things were drawn last frame: a round glow a frame behind is not seen.
pub(super) fn turn_glows(
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    lamps: Query<(&GlobalTransform, &Children), With<Lamp>>,
    mut glows: Query<&mut Transform, With<Glow>>,
) {
    let Some((_, eye)) = cameras.iter().find(|(camera, _)| camera.is_active) else {
        return;
    };
    let eye = eye.translation();
    for (lamp, children) in &lamps {
        let (_, rotation, at) = lamp.to_scale_rotation_translation();
        let Some(to_eye) = (eye - at).try_normalize() else {
            continue;
        };
        let towards = (rotation * Vec3::NEG_Z).dot(to_eye);
        // A circle faces +Z.
        let facing_eye = Quat::from_rotation_arc(Vec3::Z, to_eye);
        let placed = Transform {
            translation: Vec3::ZERO,
            rotation: rotation.inverse() * facing_eye,
            scale: Vec3::splat(glow_scale(towards)),
        };
        for &child in children {
            if let Ok(mut glow) = glows.get_mut(child) {
                glow.set_if_neq(placed);
            }
        }
    }
}

/// A soft round glow in white on black, brightest in the middle.
fn soft_glow() -> Image {
    let size = SOFT_GLOW_SIZE;
    let middle = (size as f32 - 1.0) / 2.0;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let out = Vec2::new(x as f32 - middle, y as f32 - middle).length() / middle;
            let bright = ((1.0 - out).max(0.0).powi(2) * 255.0) as u8;
            rgba.extend_from_slice(&[bright, bright, bright, 255]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        rgba,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Shows the lamps that shine now and hides the rest, and turns the beacons.
pub(super) fn light_lamps(
    time: Res<Time>,
    settings: Res<TruckLamps>,
    mut lamps: Query<(&Lamp, &mut Transform, &mut Visibility)>,
) {
    let seconds = time.elapsed_secs();
    for (Lamp { lamp, .. }, mut transform, mut visibility) in &mut lamps {
        let on = settings.lit && blinked_on(lamp.blink, seconds);
        visibility.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if on && lamp.spin != 0.0 {
            *transform = placed(lamp, seconds);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::truck::TruckData;

    #[test]
    fn a_lamp_shines_the_way_it_faces() {
        for lamp in TruckData::builtin().lamps {
            let transform = placed(&lamp, 0.0);
            assert!(transform.forward().as_vec3().abs_diff_eq(lamp.facing, 1e-5));
            assert_eq!(transform.translation, lamp.position);
        }
    }

    #[test]
    fn a_beacon_goes_round_to_the_left() {
        let beacon = TruckLamp {
            facing: Vec3::NEG_Z,
            spin: std::f32::consts::FRAC_PI_2,
            ..TruckData::builtin().lamps[0].clone()
        };
        let after_a_second = placed(&beacon, 1.0).forward().as_vec3();
        assert!(
            after_a_second.abs_diff_eq(Vec3::NEG_X, 1e-5),
            "{after_a_second}"
        );
    }

    #[test]
    fn a_cone_is_as_wide_as_its_beam_at_each_end() {
        let beam = Beam {
            reach: 20.0,
            width: 0.2,
            spread: 0.1,
        };
        let mesh = cone_mesh(beam);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        let far = 0.2 + 20.0 * 0.1f32.tan();
        for position in positions {
            let across = Vec2::new(position[0], position[1]).length();
            if position[2] == 0.0 {
                assert!((across - 0.2).abs() < 1e-5);
            } else {
                assert_eq!(position[2], -20.0);
                assert!((across - far).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn a_glow_is_seen_from_in_front_and_not_from_behind() {
        assert_eq!(glow_scale(1.0), 1.0);
        assert_eq!(glow_scale(-0.5), 0.0);
        assert!(glow_scale(0.1) >= GLOW_FROM_THE_SIDE);
        assert!(glow_scale(0.5) < glow_scale(0.9));
    }

    #[test]
    fn a_blinking_lamp_is_on_then_off() {
        let blink = Some([0.3, 0.7]);
        assert!(blinked_on(blink, 0.1));
        assert!(!blinked_on(blink, 0.5));
        assert!(blinked_on(blink, 1.2));
        assert!(blinked_on(None, 0.5));
    }
}
