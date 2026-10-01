//! The rules of the physics engine that the game relies on, each in a small world of its
//! own. They pin what `docs/avian.md` says was measured, so that an upgrade of Avian that
//! changes one of them is found here, and not by driving.

use std::f32::consts::FRAC_PI_2;
use std::time::Duration;

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

const STEP: f64 = 1.0 / 120.0;

/// Marks a collider whose contacts the test hook changes, and says how.
#[derive(Component, Clone, Copy)]
enum Change {
    Drop,
    Normal(Vec3),
    Slippery,
    Belt(Vec3),
}

#[derive(SystemParam)]
struct TestHooks<'w, 's> {
    changes: Query<'w, 's, &'static Change>,
}

impl CollisionHooks for TestHooks<'_, '_> {
    fn modify_contacts(&self, contacts: &mut ContactPair, _commands: &mut Commands) -> bool {
        // The normal points from the first collider to the second, and the marked one may
        // be either.
        let (change, sign) = match (
            self.changes.get(contacts.collider1),
            self.changes.get(contacts.collider2),
        ) {
            (Ok(&change), _) => (change, 1.0),
            (_, Ok(&change)) => (change, -1.0),
            _ => return true,
        };
        for manifold in &mut contacts.manifolds {
            match change {
                Change::Drop => return false,
                Change::Normal(normal) => manifold.normal = normal * sign,
                Change::Slippery => manifold.friction = 0.0,
                // See `a_belt_carries_a_box`.
                Change::Belt(velocity) => manifold.tangent_velocity = -velocity * sign,
            }
        }
        true
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        PhysicsPlugins::default().with_collision_hooks::<TestHooks>(),
    ))
    .init_asset::<Mesh>()
    .insert_resource(Time::<Fixed>::from_seconds(STEP))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        STEP,
    )));
    // What `App::run` does and `App::update` does not: Avian makes some of its resources
    // only here.
    app.finish();
    app.cleanup();
    app
}

fn run(app: &mut App, seconds: f64) {
    for _ in 0..(seconds / STEP).round() as usize {
        app.update();
    }
}

/// A level floor 40 m across with its top at y = 0, and a 1 m box resting on it.
fn floor_and_box(app: &mut App, floor: impl Bundle, speed: Vec3) -> Entity {
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
        floor,
    ));
    app.world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(1.0, 1.0, 1.0),
            Transform::from_xyz(0.0, 0.5, 0.0),
            LinearVelocity(speed),
        ))
        .id()
}

fn at(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<Transform>(entity).unwrap().translation
}

fn velocity(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<LinearVelocity>(entity).unwrap().0
}

#[test]
fn a_contact_the_hook_drops_holds_nothing_up() {
    let mut app = app();
    let dropped = floor_and_box(
        &mut app,
        (Change::Drop, ActiveCollisionHooks::MODIFY_CONTACTS),
        Vec3::ZERO,
    );
    run(&mut app, 1.0);
    assert!(at(&app, dropped).y < -2.0, "{}", at(&app, dropped));

    let mut app = self::app();
    let kept = floor_and_box(&mut app, (), Vec3::ZERO);
    run(&mut app, 1.0);
    assert!((at(&app, kept).y - 0.5).abs() < 0.02, "{}", at(&app, kept));
}

/// The normal the hook gives is the way the contact pushes, from the first collider to
/// the second. Turned to lean towards +X, a floor pushes a box resting on it that way.
#[test]
fn a_contact_pushes_along_the_normal_the_hook_gives() {
    let mut app = app();
    // Steeper than the friction holds.
    let leaning = Vec3::new(1.0, 1.0, 0.0).normalize();
    let pushed = floor_and_box(
        &mut app,
        (
            Change::Normal(leaning),
            ActiveCollisionHooks::MODIFY_CONTACTS,
        ),
        Vec3::ZERO,
    );
    run(&mut app, 0.5);
    let moved = at(&app, pushed);
    assert!(moved.x > 0.05, "{moved}");
}

#[test]
fn a_contact_with_no_friction_slides_freely() {
    let mut app = app();
    let slid = floor_and_box(
        &mut app,
        (Change::Slippery, ActiveCollisionHooks::MODIFY_CONTACTS),
        Vec3::X * 5.0,
    );
    run(&mut app, 1.0);
    assert!(
        (velocity(&app, slid).x - 5.0).abs() < 0.05,
        "{}",
        velocity(&app, slid)
    );

    let mut app = self::app();
    let held = floor_and_box(&mut app, (), Vec3::X * 5.0);
    run(&mut app, 1.0);
    assert!(velocity(&app, held).x < 1.0, "{}", velocity(&app, held));
}

/// `tangent_velocity` is how fast the solver lets the first collider's surface slide
/// along the second's without friction: measured, a box (first) on a still floor (second)
/// with `tangent_velocity` +3 X was carried at +3 X. Avian's own documentation says it the
/// other way round. `truck/contacts.rs` relies on this way.
#[test]
fn a_belt_carries_a_box() {
    let mut app = app();
    let carried = floor_and_box(
        &mut app,
        (
            Change::Belt(Vec3::X * 3.0),
            ActiveCollisionHooks::MODIFY_CONTACTS,
        ),
        Vec3::ZERO,
    );
    run(&mut app, 1.0);
    assert!((velocity(&app, carried) - Vec3::X * 3.0).length() < 0.05);
}

/// A heightfield of `cells` by `cells` cells, each `cell` metres, level at y = 0.
fn level_heightfield(cells: usize, cell: f32) -> Collider {
    let points = cells + 1;
    Collider::heightfield(
        vec![vec![0.0; points]; points],
        Vec3::new(cells as f32 * cell, 1.0, cells as f32 * cell),
    )
}

/// The tire's sweep: a cylinder lying on its axle, cast straight down.
#[test]
fn a_tire_swept_onto_the_ground_finds_it() {
    let mut app = app();
    app.world_mut().spawn((
        RigidBody::Static,
        level_heightfield(16, 2.0),
        Transform::default(),
    ));
    run(&mut app, STEP);

    let radius = 1.0;
    let tire = Collider::cylinder(radius, 0.8);
    let lying = Quat::from_rotation_z(FRAC_PI_2);
    let cast = |app: &mut App, from: Vec3| {
        let world = app.world_mut();
        let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
        let query = state.get(world).unwrap();
        query.cast_shape(
            &tire,
            from,
            lying,
            Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(5.0),
            &SpatialQueryFilter::default(),
        )
    };

    // From above: it travels down to where the tread touches, straight under the axle.
    let hit = cast(&mut app, Vec3::new(0.3, 3.0, 0.2)).expect("the ground");
    assert!((hit.distance - 2.0).abs() < 1e-3, "{hit:?}");
    assert!(hit.normal1.dot(Vec3::Y) > 0.999, "{hit:?}");
    // The touch is given in the world; the tire's own frame (axis along Y) is found from
    // where the tire ended up.
    let on_the_tire = lying.inverse() * (hit.point2 - Vec3::new(0.3, 1.0, 0.2));
    assert!((on_the_tire.xz().length() - radius).abs() < 1e-3);

    // From inside: it stops at once. The normal is not the ground's but a triangle's edge,
    // as it was in Rapier (the same case gave (1, 0, 0) there): `truck/drive.rs` must not
    // take it for the way out.
    let hit = cast(&mut app, Vec3::new(0.3, 0.6, 0.2)).expect("the ground");
    assert_eq!(hit.distance, 0.0);
    assert!(hit.normal1.dot(Vec3::Y) < 0.1, "{hit:?}");
}

/// A ball rolling over level ground made of triangles is thrown up by the edges between
/// them, by up to a quarter of its radius, and slowed. Rapier did the same: the same case
/// rose to 0.627 m and ended at 6.2 m/s. Any change here changes how a wheel's core rides.
#[test]
fn a_ball_rolls_smoothly_over_a_heightfield() {
    let mut app = app();
    app.world_mut().spawn((
        RigidBody::Static,
        level_heightfield(64, 1.0),
        Transform::default(),
    ));
    let ball = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.5),
            Transform::from_xyz(-20.0, 0.5, -3.0),
            LinearVelocity(Vec3::new(10.0, 0.0, 3.0)),
        ))
        .id();
    let mut highest = 0.0f32;
    for _ in 0..(3.0 / STEP) as usize {
        app.update();
        highest = highest.max(at(&app, ball).y);
    }
    assert!(highest < 0.65, "{highest}");
}

/// A child collider moved in `FixedUpdate` is where it was put in the step that follows,
/// and moving it neither moves the body nor changes its mass.
#[test]
fn a_child_collider_moved_before_the_step_is_there_in_it() {
    #[derive(Component)]
    struct Moved;

    let mut app = app();
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(1.0, 1.0, 1.0),
            Mass(100.0),
            NoAutoMass,
            NoAutoAngularInertia,
            NoAutoCenterOfMass,
            AngularInertia::new(Vec3::splat(10.0)),
            Transform::from_xyz(0.0, 5.0, 0.0),
            GravityScale(0.0),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Moved,
            ChildOf(body),
            Collider::sphere(0.25),
            ColliderDensity(0.0),
            Transform::from_xyz(2.0, 0.0, 0.0),
        ))
        .id();
    app.add_systems(
        FixedUpdate,
        |mut moved: Query<&mut Transform, With<Moved>>| {
            for mut transform in &mut moved {
                transform.translation.y -= 0.01;
            }
        },
    );
    run(&mut app, 0.5);
    let world = app.world_mut();
    let aabb = world.get::<ColliderAabb>(child).unwrap();
    let expected = 5.0 - 0.01 * (0.5 / STEP) as f32;
    let centre = (aabb.min + aabb.max) / 2.0;
    println!("child's box centre {centre}, expected y {expected}");
    assert!((centre.y - expected).abs() < 0.011, "{centre} {expected}");
    assert_eq!(at(&app, body), Vec3::new(0.0, 5.0, 0.0));
    assert_eq!(
        app.world().get::<ComputedMass>(body).unwrap().value(),
        100.0
    );
}
