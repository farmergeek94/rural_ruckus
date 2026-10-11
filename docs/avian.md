# Avian: the physics engine

The physics moved from Rapier (`bevy_rapier3d` 0.36) to Avian (`avian3d` 0.7), at the
user's request: a full rewrite of every part that touches the engine. No Rapier code or
dependency is left. This document records what maps to what, what was found, what was
decided, and the figures measured before and after.

Status: **done**, on the branch `avian`, measured headless against Rapier (section 5).
**Not yet driven.**

## 1. Summary

- Avian 0.7.0 is the release for Bevy 0.19. `src/physics.rs` sets it up for the game and
  for every test app: 6 substeps, and `truck::decide_tire_contacts` decides the tires'
  contacts (3.11).
- The truck model is unchanged (decision 1): a pushed body, a swept tire, springs on a
  beam axle, wheel colliders and a hook.
- Headless, the trucks give Rapier's figures in nearly every case. Over steps and logs
  they now do what the code's notes say was measured when the rules were made, which
  Rapier no longer did. In a two-minute race on Alpine they are as good or better.
- An update of the headless app costs 3.6 ms against 3.9 ms, and the worst 9 ms against
  26 ms.
- Eight things in Avian differ from Rapier and broke the game (section 3). Each is
  handled, and pinned by a test where it can be.

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
| `RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule()` | `PhysicsPlugins::default()`, and `decide_tire_contacts` in `NarrowPhaseSystems::Last` (3.11) | read |
| Physics runs in `FixedUpdate` | Physics runs in `FixedPostUpdate` by default | read |
| `TimestepMode::Fixed { dt, substeps: 1 }` | `Time<Fixed>` at 120 Hz, and the `SubstepCount` resource | read. The default is 6 substeps. |
| `.before(PhysicsSet::SyncBackend)` | A system in `FixedUpdate` already runs before the step. No order is necessary. | read |
| `.after(PhysicsSet::Writeback)` | Put the system in `FixedPostUpdate`, `.after(PhysicsSystems::Writeback)` | read |
| `RapierDebugRenderPlugin`, `DebugRenderContext::enabled` | Avian's `PhysicsDebugPlugin`, and `PhysicsGizmos` in Bevy's `GizmoConfigStore` | read |

Avian's `PhysicsDebugPlugin` shares its name with the game's
`physics_debug::PhysicsDebugPlugin`: use the full path. Avian moves a body's `Transform`
between steps only with `TransformInterpolation`, which the game does not add
(decision 2).

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

A force from `Forces` wakes a sleeping body unless `non_waking()` is used. A changed
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

When combine rules disagree, both engines apply `Max`, then `Min`, then `Average`.
Avian's `Friction` has a static and a dynamic coefficient; Rapier has one.

Parry puts the row of the height array along Z and the column along X; Avian's
constructor document says the opposite. Parry's `Array2` is column-major, as Rapier's
matrix was, so the heights go in as before. `tests/track.rs` checks the collider against
`HeightGrid::height_at`.

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

The hook reads the world only, as in Rapier. `suspension_takes` needs each
point in the wheel's frame (the cylinder's axis is Y). The hook moves it there with this
step's `Position`, `Rotation` and `ComputedCenterOfMass` of the truck, and the wheel
collider's `Transform`. A (wheel, heightfield ground) pair holds one manifold for each
triangle: decide for each manifold, as the Rapier code does.

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

`not_a_truck` already rejects every part of every truck, and the game has no sensors, so
the filter is the default with the predicate only.

### 2.7 Engine rules, measured

`tests/physics.rs` pins these. The Rapier comparisons used the same case in a test that
is not kept.

| Question | Answer | Mark |
| --- | --- | --- |
| The direction of `tangent_velocity` | The solver lets the **first** collider's surface slide along the second's at this velocity without friction. A box (first) on a still floor (second), with +3 X, was carried at +3 X. Avian's own document says the opposite. | measured |
| A cast that starts inside the ground | The distance is 0. The normal is a triangle's edge, not the ground's: (1, 0, 0) on level ground. Rapier gave the same normal in the same case. | measured |
| Does a ball catch on the edges between the triangles of a heightfield? | Yes: a ball of radius 0.5 rolling at 10 m/s rose to 0.627 m and slowed to 6.4 m/s. Rapier: 0.627 m and 6.2 m/s. Parry's `FIX_INTERNAL_EDGES` flag is not set, in either engine. | measured |
| Does a child collider moved in `FixedUpdate` move in the same step? | Yes. The body does not move, and with the `NoAuto` markers its mass does not change. | measured |
| A step with eight trucks' colliders on Alpine's ground, sliding at 15 m/s, with nothing else | Rapier: 1.22, 1.64, 2.62, 3.30 ms for 1, 2, 4, 6 substeps. Avian: 1.42, 1.63, 1.73, 1.92 ms. | measured |
| Headless test apps | Each must call `app.finish()` and `app.cleanup()` after its plugins are added: see 3.1. | measured |
| How hard and how fast does Avian push two shapes apart? | At most 4 m/s (`SolverConfig::max_overlap_solve_speed`), softly. The game sets it to 0 (`physics.rs`): see 3.9. | read |
| Below which speed does Avian ignore restitution? | 1 m/s (`SolverConfig::restitution_threshold`). A truck dropped from 3 m bounces as it did on Rapier. | read, and measured |
| Is `Settling` still necessary for loose scenery? | Yes, for a new reason: see 3.5. | measured |

## 3. Found during the change

Each broke something the tests or the figures showed. Each is fixed.

### 3.1 Headless apps must be finished

Avian makes some resources in `Plugin::finish`, which `App::update` does not call, so the
first step fails for a missing resource. Every test app that runs physics calls
`app.finish()` and `app.cleanup()` once its plugins are added, as `App::run` does.

### 3.2 Every collider is in the default layer

A collider with no `CollisionLayers` gets `LayerMask::DEFAULT` (bit 0). `GROUND` was bit
0 under Rapier, where a collider with no groups had no component to read, so on Avian
every rock and rail was "the ground" to the hook. The game's layers are now bits 1 to 3; a
unit test in `collision_groups.rs` keeps them off bit 0.

### 3.3 Which way `tangent_velocity` points

Avian's document says "velocity 2 minus velocity 1". Measured (`tests/physics.rs`,
`a_belt_carries_a_box`), the solver lets the **first** collider's surface slide along the
second's at this velocity without friction. `drive_contact` sets it by which collider is
the wheel: the velocity the first is to have, less the second's.

### 3.4 Contact points are halfway between the surfaces

Rapier gave each point on each collider's surface; Avian gives the halfway point. A tire
pressed 0.2 m into another truck's tire was touched 0.1 m inside its tread, below
the tread, so a rule that asked for the tread let go. Parry makes the
halfway point from the surface points and the depth, so the hook takes each surface point
back exactly (half the depth along the normal). A truck driven into another's rear tire
then rose 1.21 m, against 1.18 m on Rapier.

### 3.5 A body made asleep is never put in an island

Avian puts a body in an island with the bodies it touches in the step after it is made.
A body spawned with `Sleeping` never is, and two such bodies that touch make Avian panic
("Neither body … nor … is in an island"): measured with two touching rocks of a real
track. So loose scenery is spawned awake and put to sleep with `SleepBody` later
(`scenery/motion.rs`, `settle`), as under Rapier for another reason.

Until then the body is held (`LockedAxes::ALL_LOCKED`). A free body moves in those steps,
sleeps where it has got to, and wakes at any new contact:

- After 3 free steps, the tires stacked on Scrapyard Run (JUNK.POD), put down a little
  apart, had yet to meet. They woke as they met and rocked on their stacks all race, above
  the speed at which Avian lets a body sleep: 40 of 53 loose objects stayed awake, a step
  took 11.7 ms unoptimised where 120 steps a second leave 8.3, and the game fell further
  behind each frame: 3 frames a second.
- After 30 free steps, those tires slept, but the fences and gates of The Graveyard
  (JUNK.POD), panels 1 cm thick and 5.5 to 7.3 m high on edge on uneven ground, fell over
  first: two gates lay at 80°, fences leaned at up to 47°. With only rotation held they
  stood, but rose up to 0.97 m where the ground rises, and two overlapping gate leaves
  pushed each other 2.6 m apart.

Held for 3 steps, every loose object on every base and community track (1,471) sleeps
upright where it was put until a truck touches it.

Loose objects are no longer swept (`SweptCcd`): speculative contacts already keep them out
of the ground, and the sweep cost 4 to 5 ms a step with them awake.

### 3.6 Speculative contacts stop a tire at a kerb's face

Avian makes a contact with what a body will reach in the coming step, unless its
`SpeculativeMargin` says otherwise; Rapier made one only 2 mm ahead. The truck is built
round contacts where it overlaps something: the sweep climbs what a tire has reached into,
and the hook decides by where it is pressed in. A contact made ahead with a step's face
held the tire off it, the sweep never found the top, and the truck stopped dead: a 0.9 m
step stopped Bigfoot at 14 m/s. Trucks now have a `SpeculativeMargin` of 0
(`truck/spawn.rs`, `SPECULATION`) and ride over every step and log from 0.3 to 1.1 m at
4, 8 and 14 m/s, as the notes on `drive::UPWARD` say was measured when that rule was
made. Avian's contact tolerance of 5 mm is left.

### 3.7 A body moved by hand keeps its old pose until the next step

Avian copies a new `Transform` into `Position` and `Rotation` at the start of the next
step. `drive` works out the forces before that, and `Forces` turns the body about its
centre of mass as `Position` has it, so a truck put on the grid 200 m from its spawn was
turned about its old place and thrown end over end. `truck/reset.rs` sets `Position` and
`Rotation` with the `Transform` (`teleport`); `tests/truck.rs` watches a truck put down
200 m away for two seconds.

### 3.8 A rule the hook never applied under Rapier

`truck/contacts.rs` dropped the ground's push on a truck's body from underneath, so that a
body under the ground came back up. Rapier calls a hook only for a pair in which one
collider asks for it, and neither the body nor the ground did, so the rule never ran. On
Avian the body asked (`ActiveCollisionHooks::MODIFY_CONTACTS`) and it ran: a truck put
1.65 m into the ground was thrown out. At the user's request the rule is removed, and the
body does not ask for the hook, as under Rapier.

### 3.9 Bodies that overlap are not pushed apart

Avian pushes overlapping bodies apart at up to 4 m/s
(`SolverConfig::max_overlap_solve_speed`): two overlapping gate leaves on The Graveyard
(JUNK.POD) moved 2.6 m apart when free. At the user's request `physics.rs` sets the speed
to 0, so a contact now only stops two bodies from going further into each other. How this
changes the figures of section 5 is open.

### 3.10 A tire pushed light scenery with its whole load

`truck/drive.rs` pushes back on a body that a tire stands on. It pushed with the tire's
whole load and grip, as an impulse outside the solver, whatever the body's mass. Measured
against each loose object of the base game's tracks: a 1.4 kg cone under a wheel at
20 m/s threw the truck 51 m/s into the air; a 45 kg crate met at 10 m/s left at 1 194 m/s
and threw the truck at 52 m/s. The push now changes the body's speed by no more than the
hub closes on it along the push, plus a step of gravity (`pushed_back`): the cone threw
the truck at 8 m/s; the crate left at 6 m/s and threw it at 2 m/s. Swept CCD on the loose
objects, in either sweep mode, changed none of these.

Open: a truck driven head on at 20 to 30 m/s into a loose object of 100 to 450 kg, knee
to waist high, is often turned over. The sweep lands on the object's steep front face,
and the bump stop pushes the truck along that face's normal, which is nearly level.

### 3.11 The tires' contacts are decided by a system, not a hook

Avian's collision hooks are a type parameter of its broad and narrow phase, so with
`TireContacts` as the hook both were compiled in this crate, unoptimised in a development
build, and the hook ran for each pair in turn. `decide_tire_contacts` runs instead in
`NarrowPhaseSystems::Last`, after the narrow phase and before the solver builds its
constraints from the contact graph, and decides in parallel the pairs Avian marks from
`ActiveCollisionHooks::MODIFY_CONTACTS`. A dropped manifold is emptied of its points, not
removed: the solver has already counted each pair's manifolds, and an empty one gives it
nothing to solve. Scrapyard Run, eight trucks, unoptimised: narrow phase 0.31 ms a step
against 0.85 to 0.97, broad phase 0.07 against 0.17, whole step 2.2 ms against 2.7 to
2.9. The trucks' figures are the same but for the last decimal of a few hard hits, and
three races on Alpine were within section 5.

## 4. Decisions

The user asked the agent to take the decision that is best for Avian in each case.

1. **Keep the truck model.** A suspension from Avian's joints is a new truck, not a new
   engine, and discards every measured value. Contact friction cannot carry the tire model
   (grip from the load on each tire, the grip dial, `GroundGrip` for ice and weather, the
   lock the tires can hold, the tip guard). A cast wheel is also the usual arcade racer on
   Avian.
2. **Keep `TruckVisual`**, without Avian's `TransformInterpolation` on the body. The
   camera, the race and the lamps follow `TruckVisual`; `docs/smoothness.md` has the
   figures.
3. **Keep `tests/physics.rs`** (the engine's rules, not how a truck feels) and
   `tests/truck.rs` (a truck being moved): the only guard against an Avian upgrade that
   changes a rule the game relies on.
4. **Use Avian's default of 6 substeps.** Its solver is built round them, and they cost
   little (section 2.7).
5. **No speculative contacts for trucks** (3.6). Loose scenery keeps them, and is not swept (3.5).
6. **Do not apply the ground rule of 3.8.** The user asked for it to be removed.
7. **Leave Parry's `FIX_INTERNAL_EDGES` off** on the ground, as under Rapier. It would
   change how a wheel's core rides. Try it only if driving shows cores catching.

## 5. Figures

Measured headless by a tool in the scratch directory (not in the repository), one physics
step for each update at 120 Hz; Rapier's figures from the same tool built against the
last commit before the change. The tool drives the player's truck on a level track made
from the built-in one, except in the race on Alpine. "Bigfoot" is `trucks/99BFoot.pod`.
Where both trucks give the same figure, it is given once.

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
| Put 1.65 m into level ground | Built-in fell on through the ground. Bigfoot stayed in it | Built-in thrown out, ended 1.18 m high on its side (65°). Bigfoot stayed in it. Measured with the rule of 3.8, which is removed |
| Top speed in water after 15 s (m/s) | Built-in: under 6.9, hub-deep 15.7, tires wet 25.3. Bigfoot: 6.9, 16.9, 25.6 | The same |

Alpine, Bigfoot and seven computer-driven copies, 120 s after GO. The computer's trucks
are set up at random, so each engine ran three races; the table gives the mean.

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

## 6. Open

- **Drive it.** Every figure above is headless. The user must drive the game and say
  whether the trucks feel as they did.
- The "measured" notes in `truck/drive.rs`, `truck/contacts.rs` and `opponents.rs` were
  measured on Rapier. Those that section 5 measured again agree. The rest (for example
  the Alpine counts in `contacts.rs`) are not; the module document of `truck/contacts.rs`
  says so.
- A truck put deep into the ground, with the rule of 3.8 removed, is not measured again.
  No hub went below the ground in any race above.
- Every figure of section 5 was measured with bodies pushed apart (3.9), not with the
  speed at 0.
- Steps with a jolt over 5 g are 5% more on Avian, within the spread between races.
