# Avian: the physics engine

The game's physics moved from Rapier (`bevy_rapier3d` 0.36) to Avian (`avian3d` 0.7), by
the user's request, as a full rewrite of every part of the game that touches the engine.
This document is the record of that change: what maps to what, what was found, what was
decided, and the figures measured before and after. No Rapier code or dependency is left.

Status: **done**, on the branch `avian`. Measured headless against Rapier (section 5).
**Not yet driven**: tests and headless figures do not show that the trucks feel right.

## 1. Summary

- Avian 0.7.0 is the release for Bevy 0.19. `src/physics.rs` sets it up, with
  `truck::TireContacts` as its contact hook and 6 substeps, for the game and for every
  test app.
- The truck is the same model as before (decision 1): a body that the game pushes, with a
  swept tire, springs on a beam axle, wheel colliders and a hook.
- Headless, the trucks do what they did on Rapier to the figure in nearly every case:
  standing, dropped, accelerating, cornering, in water. Over steps and logs they now do
  what the code's notes say was measured when the rules were made, which Rapier no longer
  did. In a two-minute race on Alpine, they are as good as or better than on Rapier.
- A physics step costs less: 3.6 ms against 3.9 ms for the whole headless app, and the
  worst step 9 ms against 26 ms.
- Eight things in Avian differ from Rapier in ways that broke the game until they were
  found (section 3). Each is now handled, and the ones that can be are pinned by a test.

## 2. What maps to what

Marks, as in `docs/formats/`: **read** in the source of `avian3d` 0.7.0 or `parry3d`
0.27.0; **measured** in a test or by the tool of section 5.

### 2.1 Versions

| Item | Rapier | Avian | Mark |
| --- | --- | --- | --- |
| Bevy | 0.19.1 | 0.19.1 | read: Avian 0.7.0 depends on `bevy = "0.19.0"` |
| Physics crate | `bevy_rapier3d` 0.36.0 | `avian3d` 0.7.0 | read |
| Collision library | `parry3d` 0.30.2 | `parry3d` 0.27.0 | read. An older Parry: shape casts and contacts can differ. |
| Licence | Apache 2.0 | MIT or Apache 2.0 | read |

Avian features used: `3d`, `f32`, `parry-f32`, `parallel`, `debug-plugin`, with
`default-features = false`. The game has no use for `collider-from-mesh`, `bevy_scene`,
`bevy_picking` or `xpbd_joints`.

### 2.2 The app and the schedule

| Rapier | Avian | Mark |
| --- | --- | --- |
| `RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule()` | `PhysicsPlugins::default().with_collision_hooks::<TireContacts>()` | read |
| Physics runs in `FixedUpdate` | Physics runs in `FixedPostUpdate` by default | read |
| `TimestepMode::Fixed { dt, substeps: 1 }` | `Time<Fixed>` at 120 Hz, and the `SubstepCount` resource | read. The default is 6 substeps. |
| `.before(PhysicsSet::SyncBackend)` | A system in `FixedUpdate` already runs before the step. No order is necessary. | read |
| `.after(PhysicsSet::Writeback)` | Put the system in `FixedPostUpdate`, `.after(PhysicsSystems::Writeback)` | read |
| `RapierDebugRenderPlugin`, `DebugRenderContext::enabled` | Avian's `PhysicsDebugPlugin`, and `PhysicsGizmos` in Bevy's `GizmoConfigStore` | read |

Avian's `PhysicsDebugPlugin` has the same name as the game's `physics_debug::PhysicsDebugPlugin`.
Use the full path for Avian's.

Avian moves a body's `Transform` for interpolation only if the body has
`TransformInterpolation`. The game does not add it, so rule "never draw a physics body
directly" stays as it is.

### 2.3 Bodies

| Rapier | Avian | Mark |
| --- | --- | --- |
| `RigidBody::Fixed` | `RigidBody::Static` | read |
| `RigidBody::KinematicVelocityBased` | `RigidBody::Kinematic` with `LinearVelocity` | read |
| `Velocity { linear, angular }` | `LinearVelocity` and `AngularVelocity`, two components | read |
| `ExternalForce`. It stays between steps, so `drive` builds it again each step. | `Forces` in a query. Avian clears the forces after each step. | read |
| `ExternalImpulse` | `Forces::apply_linear_impulse_at_point`. It changes the velocity at once. | read |
| `ReadMassProperties` | `ComputedMass`, `ComputedAngularInertia`, `ComputedCenterOfMass` | read |
| `ColliderMassProperties::MassProperties` on the truck | `Mass`, `AngularInertia`, `CenterOfMass`, with `NoAutoMass`, `NoAutoAngularInertia`, `NoAutoCenterOfMass` | read |
| `ColliderMassProperties::Mass(0.0)` on a wheel | `ColliderDensity(0.0)` | read |
| `Damping` | `LinearDamping` and `AngularDamping`. The formula is the same: `1 / (1 + dt * d)`, applied in each substep. | read. No difference measured (section 5). |
| `Sleeping::disabled()` | `SleepingDisabled` | read |
| `Sleeping { sleeping: true }` | The `SleepBody` command. Do not spawn a body with `Sleeping`: see 3.5. | measured |
| `Ccd::enabled()` | `SweptCcd`. Avian also has speculative contacts, on by default: see 3.6. | read |
| Gravity 9.81 m/s² down | The same default | read |

A force from `Forces` wakes a sleeping body, unless `non_waking()` is used. A changed
`Transform` or velocity wakes it too.

### 2.4 Colliders

**The arguments of the constructors are different. This is the easiest mistake to make.**

| Rapier | Avian | Mark |
| --- | --- | --- |
| `Collider::cylinder(half_height, radius)` | `Collider::cylinder(radius, height)`: other order, full height | read |
| `Collider::cuboid(hx, hy, hz)`: half extents | `Collider::cuboid(x, y, z)`: full lengths | read |
| `Collider::ball(radius)` | `Collider::sphere(radius)` | read |
| `Collider::convex_hull(&points)` | `Collider::convex_hull(points)`, which takes a `Vec` | read |
| `Collider::trimesh(vertices, triangles)` | `Collider::try_trimesh(vertices, triangles)` | read |
| `Collider::compound(parts)` | `Collider::compound(parts)` | read |
| `Collider::heightfield(..)` and `set_cell_status(ZIGZAG_SUBDIVISION)` | Avian's constructor cannot set the diagonals. Build Parry's `HeightField`, set each cell, then `Collider::from(SharedShape::new(..))`. | read: Parry 0.27 has `set_cell_status` |
| `CollisionGroups`, `Group::GROUP_1` | `CollisionLayers`, `LayerMask`. The rule is the same: each must name the other. | read |
| `Friction`, `Restitution`, `CoefficientCombineRule` | `Friction`, `Restitution`, `CoefficientCombine` | read |
| `ColliderDisabled` | `ColliderDisabled` | read |
| `collider.raw.compute_local_aabb()` | `collider.shape().compute_local_aabb()` | read |
| `collider.cast_local_ray(..)` in tests | `collider.cast_ray(position, rotation, origin, direction, max, solid)` | read |

When two combine rules disagree, the order for the rules that the game uses is the same
in both engines: `Max`, then `Min`, then `Average`. Avian's `Friction` has a static and a
dynamic coefficient. Rapier has one.

Parry says that the row of the height array goes along Z and the column along X. Avian's
document for its own constructor says the opposite. Parry's `Array2` is column-major, as
Rapier's matrix was, so the heights go in as they did. `tests/track.rs` compares the
collider with `HeightGrid::height_at`, and passes.

### 2.5 The contact hook

| Rapier | Avian | Mark |
| --- | --- | --- |
| `BevyPhysicsHooks::modify_solver_contacts` | `CollisionHooks::modify_contacts(&self, &mut ContactPair, &mut Commands) -> bool` | read |
| `ActiveHooks::MODIFY_SOLVER_CONTACTS` | `ActiveCollisionHooks::MODIFY_CONTACTS` | read |
| Called one time for each manifold | Called one time for each pair of colliders, with all the manifolds | read |
| `solver_contacts.clear()` | Remove the manifold from `contacts.manifolds`. Return `false` to remove them all. | read |
| `*context.raw.normal` | `manifold.normal`: world space, from the first shape to the second | read. The solver uses it. |
| `*context.raw.friction`, `restitution` | `manifold.friction`, `manifold.restitution` | read |
| `contact.tangent_velocity`, for each point | `manifold.tangent_velocity`, for the manifold | measured: see 3.3 |
| `point.local_p1`, `local_p2`: on each collider's surface, in its frame | `anchor1`, `anchor2`: the point halfway between the two surfaces, from each body's centre of mass, in the world | read: see 3.4 |

The hook has read-only access to the world, as in Rapier. `suspension_takes` and
`riding_up` need each point in the wheel's own frame (the cylinder's axis is Y). The hook
moves each point into that frame with the truck's `Position`, `Rotation` and
`ComputedCenterOfMass`, which are this step's, and the wheel collider's own `Transform`.

The ground is a heightfield. One pair (wheel, ground) can hold many manifolds, one for
each triangle. Decide for each manifold, as the Rapier code does.

### 2.6 The tire sweep

| Rapier | Avian | Mark |
| --- | --- | --- |
| `ReadRapierContext`, `rapier.cast_shape(..)` | `SpatialQuery::cast_shape_predicate(..)` | read |
| The shape is Parry's `Cylinder` | The shape is a `Collider` | read |
| `max_time_of_impact` | `max_distance` | read |
| `stop_at_penetration: true` | `ignore_origin_penetration: false`, the default | read: Avian passes the inverse to Parry |
| `compute_impact_geometry_on_penetration: true` | `compute_contact_on_penetration: true`, the default | read |
| `hit.time_of_impact` | `hit.distance` | read |
| `details.normal1`: world | `hit.normal1`: world | read |
| `details.witness2`: in the tire's frame | `hit.point2`: **world space** | read. Move it into the tire's frame. |
| `QueryFilter::exclude_rigid_body(truck)` | `SpatialQueryFilter` excludes collider entities, not bodies | read |
| `.predicate(&not_a_truck)` | The `predicate` argument | read |

The predicate `not_a_truck` already rejects every part of every truck. The exclusion of
the truck's own body adds nothing, and the game has no sensors. Thus the filter can stay
the default, with the predicate only.

### 2.7 Engine rules, measured

`tests/physics.rs` pins these. The comparisons with Rapier were made with the same case in
a test that is not kept.

| Question | Answer | Mark |
| --- | --- | --- |
| The direction of `tangent_velocity` | The solver lets the **first** collider's surface slide along the second's at this velocity without friction. A box (first) on a still floor (second), with +3 X, was carried at +3 X. Avian's own document says the opposite. | measured |
| A cast that starts inside the ground | The distance is 0. The normal is a triangle's edge, not the ground's: (1, 0, 0) on level ground. Rapier gave the same normal in the same case. | measured |
| Does a ball catch on the edges between the triangles of a heightfield? | Yes: a ball of radius 0.5 rolling at 10 m/s rose to 0.627 m and slowed to 6.4 m/s. Rapier: 0.627 m and 6.2 m/s. Parry's `FIX_INTERNAL_EDGES` flag is not set, in either engine. | measured |
| Does a child collider moved in `FixedUpdate` move in the same step? | Yes. The body does not move, and with the `NoAuto` markers its mass does not change. | measured |
| A step with eight trucks' colliders on Alpine's ground, sliding at 15 m/s | Rapier: 1.22, 1.64, 2.62, 3.30 ms for 1, 2, 4, 6 substeps. Avian: 1.42, 1.63, 1.73, 1.92 ms. | measured |
| Headless test apps | Avian makes some of its resources in `Plugin::finish`. `App::update` does not call it, so each test app must call `app.finish()` and `app.cleanup()` after its plugins are added. | measured |
| How hard and how fast does Avian push two shapes apart? | At most 4 m/s (`SolverConfig::max_overlap_solve_speed`), softly. | read |
| Below which speed does Avian ignore restitution? | 1 m/s (`SolverConfig::restitution_threshold`). A truck dropped from 3 m bounces as it did on Rapier. | read, and measured |
| Is `Settling` still necessary for loose scenery? | Yes, for a new reason: see 3.5. | measured |

## 3. Found during the change

Each of these broke something that the tests or the figures showed, and is fixed.

### 3.1 Headless apps must be finished

Avian makes some of its resources in `Plugin::finish`, which `App::update` does not call.
Every test app that runs physics calls `app.finish()` and `app.cleanup()` once its plugins
are added, as `App::run` does. Without it, the first step fails for a missing resource.

### 3.2 Every collider is in the default layer

Avian gives each collider a `CollisionLayers` of `LayerMask::DEFAULT` (bit 0) when it has
none of its own. `GROUND` was bit 0 under Rapier, where a collider with no groups had no
component to read. On Avian that made every rock and rail "the ground" to the hook. The
game's layers are bits 1 to 3, and a unit test in `collision_groups.rs` keeps them off the
default one.

### 3.3 Which way `tangent_velocity` points

Avian's document says it is "velocity 2 minus velocity 1". Measured, it is the velocity
at which the solver lets the **first** collider's surface slide along the second's without
friction (`tests/physics.rs`, `a_belt_carries_a_box`). `let_it_roll` sets it by which of
the two colliders is the wheel.

### 3.4 Contact points are halfway between the surfaces

Rapier gave each point on each collider's surface. Avian gives the point halfway between
the two, so a tire pressed 0.2 m into another truck's tire was touched 0.1 m inside its
tread, below `SIDEWALL`, and so on its sidewall: the rule that lets a tire ride up another
truck let go, and the tire stopped as at a wall. Parry makes the halfway point from each
surface point and the depth, so the hook takes each surface point back exactly (half the
depth along the normal). With that, a truck driven into another's rear tire rose 1.21 m,
where it rose 1.18 m on Rapier.

### 3.5 A body made asleep is never put in an island

Avian puts a body in with the bodies it touches (its island) in the step after it is
made, and a body spawned with `Sleeping` is never put in. When two such bodies touch,
Avian panics ("Neither body … nor … is in an island"): measured with two touching rocks of
a real track. So loose scenery is made awake and put to sleep with `SleepBody` later
(`scenery/motion.rs`, `settle`), as it was for another reason under Rapier.

Not three steps later, as under Rapier: a sleeping body is woken by any contact that
starts, and objects are put down a little apart. After 3 steps, the tires stacked on
Scrapyard Run (JUNK.POD) had yet to meet each other; they woke as they met, and rocked on
their stacks for the whole race, above the speed at which Avian lets a body sleep. 40 of
53 loose objects stayed awake, a step took 11.7 ms unoptimised, where 120 steps a second
leave 8.3, and the game fell further behind each frame: 3 frames a second. After 30 steps,
every loose object on every base and community track sleeps until a truck touches it.
Loose objects are no longer swept (`SweptCcd`) either: Avian's speculative contacts
already keep them from passing through the ground, and the sweep cost 4 to 5 ms a step
with them awake.

### 3.6 Speculative contacts stop a tire at a kerb's face

Avian makes a contact with what a body will reach in the coming step, before it touches,
unless its `SpeculativeMargin` says otherwise. Rapier made one only 2 mm ahead. The truck
is built round contacts made where it overlaps something: the sweep climbs what a tire has
reached into, and the hook decides by where a tire is pressed in. Made ahead of time, the
contact with a step's face held the tire off it, the sweep never found the top, and the
truck stopped dead: a 0.9 m step stopped Bigfoot at 14 m/s. Trucks now have a
`SpeculativeMargin` of 0 (`truck/spawn.rs`, `SPECULATION`), and ride over every step and
log from 0.3 to 1.1 m at 4, 8 and 14 m/s, as the notes on `drive::UPWARD` say was measured
when that rule was made. Avian's own contact tolerance, 5 mm, is left.

### 3.7 A body moved by hand keeps its old pose until the next step

Avian takes a new `Transform` over into `Position` and `Rotation` at the start of the
next step, but `drive` works out the forces before it, and `Forces` turns the body about
its centre of mass as `Position` has it. So a truck put on the grid 200 m from where it
was spawned was turned about its old place, and thrown end over end. `truck/reset.rs` sets
`Position` and `Rotation` with the `Transform` (`teleport`), and `tests/truck.rs` watches a
truck put down 200 m away for two seconds.

### 3.8 A rule the hook never applied under Rapier

`truck/contacts.rs` drops a push of the ground on a truck's body from underneath, so that
a body that has got under the ground comes back up. Rapier calls a hook only for a pair in
which one collider asks for it, and neither the body nor the ground did, so the rule was
never applied. The body now asks (`ActiveCollisionHooks::MODIFY_CONTACTS`), and the rule is
applied.

## 4. Decisions

The user asked the agent to take, in each case, the decision that is best for Avian.

1. **Keep the truck model.** The alternative is a suspension made from Avian's joints.
   That is a new truck, not a new engine, and it discards every measured value. Grip that
   comes from contact friction cannot carry the tire model (grip from the load on each
   tire, the grip dial, `GroundGrip` for ice and weather, the lock that the tires can
   hold, the tip guard). A cast wheel is also the usual way to build an arcade racer on
   Avian.
2. **Keep `TruckVisual`**, and do not add Avian's `TransformInterpolation` to a body. The
   camera, the race and the lamps follow `TruckVisual`, and `docs/smoothness.md` has the
   figures for it.
3. **Keep `tests/physics.rs`**, headless tests of the engine's rules (not of how a truck
   feels), and `tests/truck.rs`, of a truck being moved. They are the only guard against
   an upgrade of Avian that changes a rule the game relies on.
4. **Use Avian's default of 6 substeps.** Its solver is built round them, and they cost
   little (section 2.7).
5. **No speculative contacts for trucks** (3.6). Loose scenery keeps them, and `SweptCcd`.
6. **Apply the ground rule of 3.8**, which the code already described.
7. **Leave Parry's `FIX_INTERNAL_EDGES` off** on the ground, as it was under Rapier. It
   would change how a wheel's core rides. Try it only if driving shows cores catching on
   the ground.

## 5. Figures

Measured headless by a tool in the scratch directory (not in the repository), one physics
step for each update at 120 Hz. Rapier's figures come from the same tool built against the
last commit before the change. The tool drives the player's truck itself, on a level track
made from the built-in one, except in the race on Alpine. "Bigfoot" is
`trucks/99BFoot.pod`. Where the two trucks give the same figure, it is given once.

| Case | Rapier | Avian |
| --- | --- | --- |
| At rest | Built-in 1.650 m high, Bigfoot 1.777 m, level, still | The same |
| Dropped from 3 m | 6.7 g at most, 1 bounce, still after 2.14 s | The same |
| Full throttle from rest (m/s) | 10.7 after 1 s, 29.5 after 3 s, 38.8 after 5 s, 45.0 after 10 s | The same |
| Full lock at 20 m/s, 2.5 s, built-in | Slide 7.7° mean, 11.0° at most; 1.19 g; to 14.8 m/s; tilt 11° | Slide 7.6°, 10.9°; 1.20 g; to 14.8 m/s; tilt 11° |
| Full lock at 20 m/s, 2.5 s, Bigfoot | Slide 6.1°, 8.1°; 1.14 g; to 15.1 m/s; tilt 13° | The same |
| Lock changed each second, from 15 m/s, 6 s | Built-in: slide 4.3°, 10.6°; to 8.8 m/s. Bigfoot: 2.6°, 6.8°; to 9.8 m/s | Built-in: the same, to 8.9 m/s. Bigfoot: the same |
| Steps and logs of 0.3 and 0.6 m, logs of 0.9 and 1.1 m, at 4, 8, 14 m/s | All over | All over |
| Step 0.9 m | Built-in over at 4, 8, 14 m/s. Bigfoot stopped at 4 and 8, over at 14 | Both over at 4, 8, 14 |
| Step 1.1 m | Built-in stopped at 4, over at 8 and 14. Bigfoot stopped at 4 and 8, over at 14 | Both over at 4, 8, 14 |
| Into another truck's rear tire, 8.7 m/s | Built-in: lost 2.16 m/s in a step, rose 1.18 m. Bigfoot: lost 3.64, rose 0.05 m | Built-in: lost 2.16, rose 1.21 m. Bigfoot: lost 3.18, rose 0.07 m |
| Put 1.65 m into level ground | Built-in fell on through the ground. Bigfoot stayed in it | Built-in thrown out, ended 1.18 m high on its side (65°). Bigfoot stayed in it |
| Top speed in water after 15 s (m/s) | Built-in: under 6.9, hub-deep 15.7, tires wet 25.3. Bigfoot: 6.9, 16.9, 25.6 | The same |

Alpine, Bigfoot and seven copies driven by the computer, 120 s after GO. The computer's
trucks are set up at random for each race, so each engine ran three races; the table gives
the mean.

| Figure | Rapier | Avian |
| --- | --- | --- |
| Times a truck tipped past 60° | 16.3 | 15.7 |
| Samples tipped past 60°, of 9600 | 409 | 340 |
| Steps with a jolt over 5 g | 1181 | 1243 |
| Wheel steps with a hub below the ground | 0 | 0 |
| Sudden losses of speed (> 3 m/s in a step) with no truck within 10 m | 21, 210 m/s in all | 16, 121 m/s in all |
| Trucks put back at a checkpoint | 4 | 2 |
| Checkpoints passed, all trucks | 21.7 | 23 |
| Mean speed | 21.1 m/s | 22.2 m/s |
| Time for one update of the headless app (no drawing), one race alone | Mean 3.93 ms, 95th percentile 5.15, worst 25.81 | Mean 3.58 ms, 95th percentile 4.98, worst 9.46 |

A step with eight trucks' colliders on Alpine's ground, sliding at 15 m/s, with nothing
else: Rapier 1.22, 1.64, 2.62, 3.30 ms for 1, 2, 4, 6 substeps; Avian 1.42, 1.63, 1.73,
1.92 ms.

## 6. Open

- **Drive it.** Every figure above is headless. The user must drive the game and say
  whether the trucks feel as they did.
- The "measured" notes in `truck/drive.rs`, `truck/contacts.rs` and `opponents.rs` give
  figures that were measured on Rapier. Those that section 5 measured again agree. The
  others (for example the Alpine counts in `contacts.rs`) are not measured again; the
  module document of `truck/contacts.rs` says so.
- A truck put deep into the ground (3.8) is thrown out hard on Avian, where on Rapier it
  fell through. Neither happens in a race: no hub went below the ground in any race above.
- Steps with a jolt over 5 g are 5% more on Avian, within the spread between races.
