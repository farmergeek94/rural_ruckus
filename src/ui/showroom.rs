//! The showroom behind the screens: a polished floor, a chevron as wide as the view standing
//! behind the truck with its point over it, three lights and a camera. The game
//! parents whatever is to be shown to the `Turntable`, which is not drawn: it hangs in the
//! air to the right of centre and rises and falls a little, so that the truck floats.
//!
//! The truck turns by itself. Drag anywhere that isn't a panel, or push the right
//! stick, and it follows instead; let go and it eases back to turning by itself.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::FrontEndOpen;

/// How fast the turntable turns when left alone, in radians a second: 20 degrees.
const TURNING_RATE: f32 = 0.349;
/// How quickly it comes back to that rate once let go, per second: at 6, nearly all the
/// way in half a second. Higher snaps back; lower coasts.
const EASE_BACK: f32 = 6.0;
/// The fastest it can be left spinning by a flick, in radians a second.
const FASTEST_SPIN: f32 = 12.0;
/// Radians of turn for each pixel of drag.
const DRAG_SENSITIVITY: f32 = 0.008;
/// Radians a second with the right stick hard over.
const STICK_RATE: f32 = 3.0;
/// Below this a stick is taken to be at rest.
const STICK_AT_REST: f32 = 0.15;

/// How high the truck's tires float over the floor, in metres, on average.
const HOVER_HEIGHT: f32 = 1.0;
/// How far it rises and falls about that, in metres, and how long one rise and fall takes,
/// in seconds. Bigger or quicker looks like bobbing on water rather than floating.
const HOVER_RISE: f32 = 0.12;
const HOVER_PERIOD: f32 = 4.0;
/// Where the camera is and what it looks at, in metres. It looks past the truck, which is
/// over the origin, to its left, which is what puts the truck in the right two thirds of
/// the picture. A little above hub height.
const CAMERA_POSITION: Vec3 = Vec3::new(-2.0, 2.6 + HOVER_HEIGHT, 14.5);
const CAMERA_LOOKS_AT: Vec3 = Vec3::new(-4.6, 1.5 + HOVER_HEIGHT, 0.0);

/// How far behind the truck the chevron stands, in metres, and how high its point is.
const CHEVRON_DEPTH: f32 = 14.0;
const CHEVRON_APEX: f32 = 7.5;
/// How far each arm drops for each metre it runs out sideways. Steeper looks more like an
/// arrow and less like a roof line.
const CHEVRON_SLOPE: f32 = 0.12;
/// How far each arm runs out sideways, in metres: past the edges of the widest screen.
const CHEVRON_HALF_WIDTH: f32 = 60.0;
/// The depth of the band, measured straight down, in metres.
const CHEVRON_THICKNESS: f32 = 3.0;
/// The colours down the band, from its top edge (0) to its bottom edge (1). A narrow bright
/// strip at the top and a quick fall to a deep blue is what makes it look glossy.
const CHEVRON_SHADES: [(f32, Color); 5] = [
    (0.0, Color::srgb(0.62, 0.8, 1.0)),
    (0.035, Color::srgb(0.5, 0.72, 1.0)),
    (0.07, Color::srgb(0.16, 0.45, 0.97)),
    (0.45, Color::srgb(0.07, 0.25, 0.72)),
    (1.0, Color::srgb(0.02, 0.08, 0.3)),
];
/// A thin line over the band, running beside it, and how far above it is, in metres.
const STRIPE_GAP: f32 = 1.6;
const STRIPE_THICKNESS: f32 = 0.35;
const STRIPE_SHADES: [(f32, Color); 3] = [
    (0.0, Color::srgb(0.55, 0.75, 1.0)),
    (0.3, Color::srgb(0.2, 0.47, 0.95)),
    (1.0, Color::srgb(0.06, 0.2, 0.6)),
];

/// Whatever is a child of this turns and floats with it. A child's feet go at its origin.
#[derive(Component, Debug)]
pub struct Turntable {
    /// How fast it turns when left alone, in radians a second.
    pub rate: f32,
    /// Whether the pointer has hold of it.
    pub held: bool,
    /// How fast it is turning now.
    speed: f32,
    /// Turn asked for by whoever holds it, and not yet made, in radians.
    pushed: f32,
}

impl Default for Turntable {
    fn default() -> Self {
        Self {
            rate: TURNING_RATE,
            held: false,
            speed: TURNING_RATE,
            pushed: 0.0,
        }
    }
}

impl Turntable {
    /// Turn it by hand, by `radians`. Counts as holding it for this frame.
    pub fn push(&mut self, radians: f32) {
        self.pushed += radians;
    }

    /// How far to turn over the next `seconds`, in radians. `held` follows the hand, and
    /// remembers how fast it was going, so that letting go carries on from there.
    fn advance(&mut self, held: bool, seconds: f32) -> f32 {
        if seconds <= 0.0 {
            return 0.0;
        }
        if held {
            let turn = std::mem::take(&mut self.pushed);
            self.speed = (turn / seconds).clamp(-FASTEST_SPIN, FASTEST_SPIN);
            return turn;
        }
        self.pushed = 0.0;
        self.speed += (self.rate - self.speed) * (1.0 - (-EASE_BACK * seconds).exp());
        self.speed * seconds
    }
}

/// Takes the drags that land on nothing else: it fills the window, behind every panel.
#[derive(Component)]
pub(super) struct DragSurface;

pub(super) fn spawn_showroom(
    mut commands: Commands,
    // Both absent in a headless app, where the showroom is a turntable and nothing to see.
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let scoped = DespawnOnExit(FrontEndOpen::Open);

    commands.spawn((
        Name::new("Turntable"),
        Turntable::default(),
        Transform::from_xyz(0.0, HOVER_HEIGHT, 0.0),
        Visibility::default(),
        scoped.clone(),
    ));

    if let (Some(mut meshes), Some(mut materials)) = (meshes, materials) {
        commands.spawn((
            Name::new("Showroom floor"),
            Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
            // Dark and blue, with a soft sheen, so that it catches the lights and the truck's shadow.
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.02, 0.04, 0.09),
                perceptual_roughness: 0.55,
                reflectance: 0.2,
                ..default()
            })),
            Transform::default(),
            scoped.clone(),
        ));

        // Unlit, so that its colours are exactly the ones it is given, whatever the lights do.
        let backdrop = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Name::new("Chevron"),
            Mesh3d(meshes.add(chevron(CHEVRON_APEX, CHEVRON_THICKNESS, &CHEVRON_SHADES))),
            MeshMaterial3d(backdrop.clone()),
            Transform::from_xyz(0.0, 0.0, -CHEVRON_DEPTH),
            scoped.clone(),
        ));
        commands.spawn((
            Name::new("Chevron stripe"),
            Mesh3d(meshes.add(chevron(
                CHEVRON_APEX + STRIPE_GAP + STRIPE_THICKNESS,
                STRIPE_THICKNESS,
                &STRIPE_SHADES,
            ))),
            MeshMaterial3d(backdrop),
            Transform::from_xyz(0.0, 0.0, -CHEVRON_DEPTH),
            scoped.clone(),
        ));
    }

    // A key light from the camera's side, which casts the shadows, a fill from the other
    // to keep them from going black, and a rim from behind to cut the truck out of the dark.
    let looking_at_the_truck = |from: Vec3| {
        Transform::from_translation(from)
            .looking_at(Vec3::new(0.0, 1.5 + HOVER_HEIGHT, 0.0), Vec3::Y)
    };
    commands.spawn((
        Name::new("Key light"),
        SpotLight {
            intensity: 14_000_000.0,
            range: 60.0,
            outer_angle: 0.75,
            inner_angle: 0.5,
            shadow_maps_enabled: true,
            ..default()
        },
        looking_at_the_truck(Vec3::new(7.0, 11.0, 10.0)),
        scoped.clone(),
    ));
    commands.spawn((
        Name::new("Fill light"),
        SpotLight {
            intensity: 5_000_000.0,
            range: 60.0,
            outer_angle: 0.9,
            inner_angle: 0.6,
            color: Color::srgb(0.75, 0.85, 1.0),
            ..default()
        },
        looking_at_the_truck(Vec3::new(-11.0, 5.0, 8.0)),
        scoped.clone(),
    ));
    commands.spawn((
        Name::new("Rim light"),
        SpotLight {
            intensity: 8_000_000.0,
            range: 60.0,
            outer_angle: 0.7,
            inner_angle: 0.4,
            color: Color::srgb(0.45, 0.7, 1.0),
            ..default()
        },
        looking_at_the_truck(Vec3::new(-3.0, 8.0, -12.0)),
        scoped.clone(),
    ));

    commands.spawn((
        Name::new("Showroom camera"),
        Camera3d::default(),
        Camera {
            // The showroom's walls are a deep blue darkness.
            clear_color: ClearColorConfig::Custom(Color::srgb(0.008, 0.015, 0.035)),
            ..default()
        },
        Transform::from_translation(CAMERA_POSITION).looking_at(CAMERA_LOOKS_AT, Vec3::Y),
        scoped.clone(),
    ));

    commands.spawn((
        Name::new("Drag surface"),
        DragSurface,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        GlobalZIndex(-1),
        scoped,
    ));
}

pub(super) fn dragged(
    drag: On<Pointer<Drag>>,
    surfaces: Query<(), With<DragSurface>>,
    mut turntables: Query<&mut Turntable>,
) {
    if !surfaces.contains(drag.entity) {
        return;
    }
    for mut turntable in &mut turntables {
        turntable.held = true;
        turntable.push(drag.delta.x * DRAG_SENSITIVITY);
    }
}

pub(super) fn let_go(
    drag: On<Pointer<DragEnd>>,
    surfaces: Query<(), With<DragSurface>>,
    mut turntables: Query<&mut Turntable>,
) {
    if !surfaces.contains(drag.entity) {
        return;
    }
    for mut turntable in &mut turntables {
        turntable.held = false;
    }
}

pub(super) fn turn(
    time: Res<Time>,
    gamepads: Query<&Gamepad>,
    mut turntables: Query<(&mut Turntable, &mut Transform)>,
) {
    let seconds = time.delta_secs();
    let stick: f32 = gamepads
        .iter()
        .map(|gamepad| gamepad.right_stick().x)
        .filter(|push| push.abs() > STICK_AT_REST)
        .sum();
    for (mut turntable, mut transform) in &mut turntables {
        if stick != 0.0 {
            turntable.push(stick * STICK_RATE * seconds);
        }
        let held = turntable.held || stick != 0.0;
        let turned = turntable.advance(held, seconds);
        transform.rotate_y(turned);
        transform.translation.y = hover_height(time.elapsed_secs());
    }
}

/// A flat chevron, pointing up, facing +Z, with its point at `apex` over the origin: a band
/// `thickness` deep, coloured down its depth by `shades` (fraction of the depth, colour).
fn chevron(apex: f32, thickness: f32, shades: &[(f32, Color)]) -> Mesh {
    let drop = CHEVRON_HALF_WIDTH * CHEVRON_SLOPE;
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    // Three vertices a row, left end, point and right end, and one row for each shade.
    for &(depth, color) in shades {
        let y = apex - depth * thickness;
        let color = LinearRgba::from(color).to_f32_array();
        for x in [-CHEVRON_HALF_WIDTH, 0.0, CHEVRON_HALF_WIDTH] {
            positions.push([x, if x == 0.0 { y } else { y - drop }, 0.0]);
            colors.push(color);
        }
    }
    let mut indices = Vec::new();
    for row in 0..shades.len() as u32 - 1 {
        let (top, below) = (row * 3, row * 3 + 3);
        // Each arm is a quad between this row and the next, wound to face +Z.
        for (outer, inner) in [(0, 1), (2, 1)] {
            let quad = [top + outer, below + outer, below + inner, top + inner];
            if outer == 0 {
                indices.extend([quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
            } else {
                indices.extend([quad[0], quad[2], quad[1], quad[0], quad[3], quad[2]]);
            }
        }
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// How high the truck floats at `seconds` since the start, in metres.
fn hover_height(seconds: f32) -> f32 {
    HOVER_HEIGHT + HOVER_RISE * (seconds * std::f32::consts::TAU / HOVER_PERIOD).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    #[test]
    fn turns_at_its_rate_when_left_alone() {
        let mut turntable = Turntable::default();
        let turned: f32 = (0..60).map(|_| turntable.advance(false, FRAME)).sum();
        assert!((turned - TURNING_RATE).abs() < 1e-4, "{turned}");
        assert_eq!(turntable.advance(false, 0.0), 0.0);
    }

    #[test]
    fn follows_the_hand_while_held() {
        let mut turntable = Turntable::default();
        turntable.push(0.1);
        turntable.push(0.05);
        assert!((turntable.advance(true, FRAME) - 0.15).abs() < 1e-6);
        // A hand that holds it still holds it still.
        assert_eq!(turntable.advance(true, FRAME), 0.0);
        assert_eq!(turntable.advance(true, FRAME), 0.0);
    }

    #[test]
    fn eases_back_to_its_rate_and_never_jumps() {
        let mut turntable = Turntable::default();
        // Flung backwards, at three radians a second.
        turntable.push(-3.0 * FRAME);
        let mut last = turntable.advance(true, FRAME);
        for _ in 0..30 {
            let turned = turntable.advance(false, FRAME);
            // Each frame's turn is a little nearer the rate than the last, and no more
            // than the easing allows.
            assert!(turned > last && turned - last < 3.5 * FRAME * EASE_BACK * FRAME * 1.1);
            last = turned;
        }
        // Half a second on, it is all but back.
        assert!(
            (last / FRAME - TURNING_RATE).abs() < 0.2,
            "{}",
            last / FRAME
        );

        // However hard it is flung, it isn't left spinning faster than it may.
        turntable.push(100.0);
        turntable.advance(true, FRAME);
        assert!(turntable.advance(false, FRAME) <= FASTEST_SPIN * FRAME);
    }
}
