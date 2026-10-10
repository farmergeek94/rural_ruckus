# Roadmap

What to work on next to make the game a full replacement for *Monster Truck Madness 2*
(MTM2), and every feature with its status.

Two goals, and each stage serves one or both:

- **POD compatibility**: every truck, track, model and texture in an original or
  community `.POD` works in this game.
- **Feature parity**: a player of MTM2 finds the same game here.

Work in the order of the stages. Do not start a later stage while an earlier one is not
done, unless the user asks. When you start or finish an item, update its stage here and
its row in [Feature status](#feature-status).

## Done: stages 1 to 10

| Stage | What | Status |
| --- | --- | --- |
| 1 | Driving sandbox: terrain, cast-wheel truck, chase camera, HUD | Done |
| 2 | Track and checkpoints: course, ordered gates, laps, timing, back to the last checkpoint | Done |
| 3 | POD tracks, first pass: archive, level, heightmap, texture map, track file | Done |
| 4 | Ground textures: texture list, 8-bit textures, palettes | Done |
| 5 | Models: `.BIN`, scenery with colliders, checkpoints as wide as their models | Done |
| 6 | Scenery that moves, and the rest of the look: knock-over objects, sky, animated textures | Done, not yet looked at in the game. Stage 11 finishes it. |
| 7 | Base game archives: mount several PODs at once | Done |
| 8 | Trucks: `.TRK`, body, tires, axles, wheel positions | Done, for trucks that carry their own parts |
| 9 | Race flow: front end and garage, start sequence, finish and results, cockpit and look-round views | Done, not yet driven. Stage 11 finishes it. |
| 10 | Computer trucks: a driver that follows the course | Done, first pass |

Outside the stages: the physics engine is Avian in place of Rapier, by the user's request.
Measured headless against Rapier, not yet driven. See [avian.md](avian.md).

## Next: stages 11 to 16

Many items are **open**: we do not know what MTM2 does. First find the fact in a real file
or a cited reference and record it in [formats/](formats/README.md); build it only after
that. For an item marked **Game**, first confirm what MTM2 does and record it here. Do not
invent details (see "POD compatibility rules" in `AGENTS.md`).

### Stage 11: finish what is built

Goal: what the game has now works, and the user has seen it work.

| Item | Status |
| --- | --- |
| Pause screen | Done, not yet looked at |
| Objects of type 8 turn to face the camera | Done, not yet looked at |
| Look at these in the game and record what you find: animated models, animated textures, sky, truck lights, dashboard, camera views, start sequence, results screen, frozen water, truck weight | Not yet looked at |

Done when each item is driven or looked at, with its row updated.

### Stage 12: the rest of the track files

Goal: the game uses all the data a track POD holds for the look and the race. POD
compatibility.

| Item | Status |
| --- | --- |
| Track and truck names from the POD, in the front end: the description file the `.LVL` names | Missing |
| Fog from the track: the fog file the `.LVL` names | Missing. Format **open**. |
| Lighting from the track: sun direction, shadow strength, lighting table (`.LTE`) | Missing. Only **reference**. |
| Box types 0, 1 and 11: solid or not | **Open** |
| Cylinders and the stadium in the track file | **Open** |
| Computer trucks' speed hints: `cspeed`, `ctype`, `cTrackWidth` | **Open** |
| Extra courses on one track (`Extended Course Definitions`) | Done: read, and followed by difficulty. What MTM2 does on a track with no `[Course 2]` is **open**. |
| `Track Race Type` | **Open**. Needed by stage 15. |

Done when each item is built, or its format is recorded as **open** after a search of the
references and of real files.

### Stage 13: trucks that differ

Goal: each truck is what its POD makes it. POD compatibility.

| Item | Status |
| --- | --- |
| Handling per truck: where MTM2 keeps mass, engine, springs and grip, if they vary | **Open**. The `.TRK` holds none. |
| What MTM2 does with scrape points | **Open**. Today they make the collider. |
| What each light type means (for example, brake lights only when braking) | **Open** |
| The rest of the dashboard: mirror, shifter, shift light | **Open** |

Done when each item is built, or recorded as **open** after a search.

### Stage 14: ground types

Goal: the ground acts as it looks. Mud, sand and water change how a truck drives, as the
`.TTY` says.

| Item | Status |
| --- | --- |
| Ground types other than ice: speed and grip | **Open**. What each shows is known. |

### Stage 15: the rest of the game

Goal: feature parity. Confirm each item in MTM2 before you build it (**Game**).

| Item | Status |
| --- | --- |
| Race types other than circuit (rally, Summit Rumble, others) | Missing. Needs `Track Race Type` from stage 12. |
| Championship or season across several tracks | Missing |
| Difficulty of the computer's drivers | Partial, by the user's request: easy, normal and hard (see the feature row). |
| Map of the track in the race | Done. M shows it; not yet seen in the game. |
| Other camera views (bumper, far) | Partial |
| Damage | **Open** |
| Gears: automatic and manual | Done, by the user's request: three forward gears and a reverse, automatic by default (see the feature row). |
| Force feedback | Missing |

### Stage 16: rare files

Goal: POD compatibility for files that few or no known PODs use. Do this last.

| Item | Status |
| --- | --- |
| MTM1 tracks | **Open** |
| Caves and second layers (`.RA2` to `.RA5`, `.CL1`, `.CL2`) | **Open**. No MTM2 track is known to use them. |
| Pitch and roll of placed objects | **Open**. No object examined has them. |

## Out of scope for now

Part of MTM2, but not wanted until the user asks:

- Audio: every sound and all music but the engines (the rows below marked Out of scope).
- Multiplayer (network play).
- Replays.
- A track editor. Community tracks were made with other tools, such as Traxx.

## Feature status

| Status | Meaning |
| --- | --- |
| **Done** | Works, for every POD examined so far. |
| **Partial** | Works in part. The row says what is missing. |
| **Missing** | Not started. |
| **Open** | We do not know what MTM2 does. Find out before you build it. |
| **Own** | The game has it, but the values are the game's own, not measured from MTM2. |
| **Out of scope** | Not wanted until the user asks for it. |

**Evidence** says how we know that MTM2 has the feature:

- **File**: a POD holds data for it. The link goes to the format document.
- **Game**: from how MTM2 plays, not from its files. Nobody has checked these against the
  real game for this project. **Confirm each one before you build it**, and record what
  you find here.

### Content: loading PODs

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Read a `.POD` archive | Done | File: [pod.md](formats/pod.md) | |
| Load a track POD from the command line or the front end | Done | | |
| Load a truck POD | Done | File: [truck.md](formats/truck.md) | |
| The base game's own tracks and trucks in the front end's lists | Done | File: [pod.md](formats/pod.md) | 15 tracks in 13 archives and 20 trucks in `TRUCK2.POD`. All load. Not yet on the command line, which takes archive paths. |
| Community Patch 3 true-colour textures (PNG, TGA) | Done | File: [textures.md](formats/textures.md) | `crates/pod/src/hd_texture.rs` |
| Mount the base game's archives behind a track or truck | Done | File: [pod.md](formats/pod.md) | `crates/pod/src/base_game.rs`. Only the archives that hold a borrowed file are read. What MTM2 does with the order in `POD.INI` is **open**; no two base archives disagree on a file. |
| Files that a track or truck names but does not contain | Done | File: [pod.md](formats/pod.md) | All found in the base archives for the 12 tracks and 12 trucks examined, except one model that Critic names and no archive holds. |
| Truck textures with no palette beside them | Done | File: [truck.md](formats/truck.md) | Reference: MTM2 uses `METALCR2.ACT` from its own archives. So does the game. |
| MTM1 tracks | Open | File: [level.md](formats/level.md) | The first line of a `.LVL` may tell MTM1 from MTM2. Its values are open. |

### The track world

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Terrain heights | Done | File: [terrain.md](formats/terrain.md) | |
| The world repeats beyond the edges of the map | Done | File: [terrain.md](formats/terrain.md), [situation.md](formats/situation.md) | Monte Carlo's course goes across two edges. The course is joined the short way, a truck that goes over an edge is moved across with its camera, and checkpoints, the computer's drivers, the compass and the map measure the short way. The ground, ground boxes, fixed scenery and water are drawn again 1 km past each edge; what MTM2 showed there is **open**. Driven headless across both of Monte Carlo's edges; not yet on screen. |
| Ground textures, laid per cell | Done | File: [textures.md](formats/textures.md) | Whether the whole picture is mirrored is **open**. |
| Bridges and tunnel roofs (ground boxes) | Done | File: [ground_boxes.md](formats/ground_boxes.md) | How textures lie on their sides and bottoms is **open**. |
| Caves and second layers (`.RA2` to `.RA5`, `.CL1`, `.CL2`) | Open | File: [ground_boxes.md](formats/ground_boxes.md) | No MTM2 track is known to use them. |
| Scenery models, fixed and solid | Done | File: [model.md](formats/model.md) | |
| Which box types are solid | Partial | File: [situation.md](formats/situation.md) | Types 0, 1 and 11 are **open**. Everything except 6, 7 and 8 is solid. |
| Objects with a mass that can be knocked over | Done | File: [model.md](formats/model.md) | 91 of Alpine's 323 objects have a mass. Moving objects too. `src/scenery/motion.rs`. |
| Animated models (Order and Jump records, animation control files) | Own | File: [model.md](formats/model.md) | Stage 6. Order and Jump are stepped over (Alpine's helicopter reads every face). Animation control files (`REX.BIN` in Crazy '98, `PUMPJACK.BIN` in Tinhorn Junction, `OP88ANIM.BIN` in Critic) are read (**reference**, **measured**); how they move is the game's own: in a straight line from frame to frame, round and round (`src/scenery/animation.rs`). How MTM2 moved them, and what their rate means, is **open**. Solid as the first frame. Tinhorn Junction's pump jacks stood still when they came into view after the start, a fault in Bevy 0.19; `refresh_morphs_in_view` works round it (measured offscreen). Not yet looked at in the game. |
| Animated textures | Own | File: [model.md](formats/model.md) | Stage 6. The frames are read (**reference**, **measured**); the stepping is the game's own, with the rate read as 16.16 seconds per frame, which no file shows (**open**). Scenery steps through a cycle's tiles (`shaders/tiles.wgsl`), trucks switch their material's texture (`truck/looks.rs`). The backdrop shows the first frame. Not yet looked at in the game. |
| Objects that always face the camera (type 8) | Done | File: [situation.md](formats/situation.md) | Not solid. Turned about the vertical to the camera every frame (`src/scenery/facing.rs`). Which side MTM2 shows, and about which point it turns, are **open**. Type 9 (a facing tree with a solid trunk) does not turn yet. Not yet looked at in the game. |
| Pitch and roll of placed objects | Open | File: [model.md](formats/model.md) | No object examined has them. |
| Backdrop round the horizon | Done | File: [situation.md](formats/situation.md) | `backdropType` is **open** (0 on every track). |
| Sky | Done | File: [level.md](formats/level.md) | `src/sky.rs`. The track's own picture in clear weather by day, the base game's cloudy one when overcast by day, its dusk one at a clear dusk, its night one at night; none at an overcast dusk or in fog, rain, storm or snow. How MTM2 laid the picture on the sky is **open**: the dome is the game's own. Not yet looked at in the game. |
| Fog, from the track | Missing | File: [level.md](formats/level.md), [terrain.md](formats/terrain.md) | The `.LVL` names a fog file. Format **open**. The weather's fog is the game's own. |
| Lighting: sun direction, shadow strength, lighting table (`.LTE`) | Missing | File: [level.md](formats/level.md) | Line 18 of the `.LVL`; only reference. The sun is the game's own. |
| Water | Own | File: [level.md](formats/level.md) | The height is measured. How MTM2 drew water and moved trucks in it is not. The moving water mirrors the scenery in the picture (screen-space reflections); `WaterSettings::reflections` turns it off. Since 2026-10-10 the water round the camera is a field of heights stepped on the GPU (`water/field.rs`): bow waves, wakes that persist and reflect off the shore, and a foam trail; its values are chosen from headless renders of Baja Beach, not yet by driving. |
| Frozen water in snow | Own | | In snow the water is a solid sheet of ice at its level, gripping as ice. F7 freezes and thaws it in a race. MTM2 has no known frozen water. Not yet driven. |
| Ground types (`.TTY`): ice | Own | File: [texture_types.md](formats/texture_types.md) | Only ice is used. `ICE_GRIP` is the game's own. |
| Ground types (`.TTY`): loose ground | Own | File: [texture_types.md](formats/texture_types.md) | The hundreds of the base game's types are measured: 1 road, 2 dirt, 3 water, 4 mud, 5 sand, 6 grass, 7 rocky ground, 10 metal, 12 rock, 14 railway track. Tires throw dirt only on 2 and 4 to 7, and, on ground the list does not name, where it is coloured and not blue (the game's own rule). Not yet driven. |
| Ground types: the other hundreds (sounds, spray, speed: mud, sand, water) | Open | File: [texture_types.md](formats/texture_types.md) | What each shows is known; what MTM2 did with it is not. |
| Ramps with a model (`RAMP.BIN`, `CRURAMP.BIN`) | Done | File: [situation.md](formats/situation.md) | Read from the Ramps section, placed and made solid as a box with a model is. Measured on Arizona. Not yet driven. |
| Ramps with no model | Done | File: [situation.md](formats/situation.md) | An invisible, fixed, solid wedge. How to read its sizes is measured on Torture Pit, and disagrees with the reference. Sidewinder Canyon has 8 under its skeletons, Torture Pit 1. Not yet driven. |
| Cylinders and the stadium in the track file | Open | File: [situation.md](formats/situation.md) | |
| Track music | Out of scope | File: [level.md](formats/level.md) | The `.LVL` names it. See Audio. |

### Trucks

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Body, tires, axles, bars, shocks, driveshaft | Done | File: [truck.md](formats/truck.md) | |
| Wheel positions and size | Done | File: [truck.md](formats/truck.md) | |
| Scrape points | Partial | File: [truck.md](formats/truck.md) | The convex hull of the collider. What MTM2 does with them is **open**. |
| Handling per truck (mass, engine, springs, grip) | Open | File: [truck.md](formats/truck.md) | The `.TRK` holds none. Every truck drives like the built-in one. Where MTM2 keeps handling, if it varies, is **open**. |
| Truck weight | Own | | 5 443 kg (12 000 lb), what a real monster truck weighs; MTM2's is not known. The forces were scaled with it from the 2 000 kg tuning, so the truck rides and drives as it did. Scenery with a mass, knocked by its tires, is pushed 2.7 times as hard. Not yet driven. |
| Lights (headlights, brake lights) | Partial | File: [truck.md](formats/truck.md) | Every light is read. Each lamp's picture glows at the lamp (with bloom); each beam is drawn as MTM2's cone and is a spot light, wider and longer than the cone so that it lights the course; all on at dusk, at night, and in rain and storms (`src/truck/lamps.rs`). The cone is seen only at night. Every truck lights the road: headlights with no beam (Maximum Destruction) get the base game's usual one, and a truck with no lights (8 of the 12 community trucks examined) gets the light alone. Glows face the camera. What each light type means is **open**. The built-in truck's lamps and the brightness are the game's own. Not yet looked at in the game. |
| Truck sounds | Out of scope | File: [sound.md](formats/sound.md) | `Wave File`: three sound names, each a voice saying the truck's name (**measured** by ear). Not used. |
| Dashboard (instrument cluster) | Partial | File: [cockpit.md](formats/cockpit.md) | `Instrument Cluster` names the layout (`POWERBIG.480`, in the base game's `COCKPIT.POD`), read with its pictures. In the cockpit view, the picture for the way the player looks is stretched over the window, and the road shows through palette index 0 (**measured**). Looking ahead: the steering wheel picture for the steering, and a speedometer and a tachometer needle at the file's angles (**measured**: clockwise from straight up). Not drawn: the mirror, the shifter, the shift light (**open**). Not yet looked at in the game. |
| Damage | Open | Game | Confirm whether MTM2 has it. |
| Gears: automatic and manual | Own | | `src/truck/gearbox.rs`. Three forward gears, by the user's request (five at first). The manual has a neutral below first. In neutral the throttle revs the engine freely, through the engine model's all-speed governor. Reverse goes in by itself. The gearbox is automatic by default. The manual is an option (OPTIONS > Gears, `--manual`) for the player's truck only. The gearbox chooses the gear and runs the shift timer: 0.1 s with the clutch open, while the engine lifts. It also decides how the clutch couples the engine to the wheels (`ClutchMode`). The clutch is open while a gear changes and in neutral. It is a torque converter in the automatic, in first, second and reverse, and when a manual driver coasts in a high gear. It is a driver's clutch when the manual driver presses the throttle in third. Lugging and stalling come from the engine model's torque curve and clutch, not from a rule here (see the row 'Engine and drivetrain simulation'). In third from a standstill the engine stalls under 500 rpm, about 9 m/s (12 m/s under the old rule). It runs again when the throttle is let go or the gear is changed. Each gear meets the governed engine speed (3200 rpm) at its share of the top speed (0.35, 0.65, 1.0). The automatic changes up at 0.92 of that, and down at 0.8 of the top of the gear below. **Measured** headless on 2026-10-09 with the built-in truck at full throttle: 1 to 2 at 19.3 m/s and 2923 rpm, 2 to 3 at 35.9 m/s and 2928 rpm. The gear is shown beside the speed. The tachometer reads `Engine::revs`. The gear ratios and shift points are the game's own. What MTM2 offers is **open**. The shifter pictures and the shift light of the cockpit are not drawn. Not yet driven. |
| Engine and drivetrain simulation | Own | | `src/truck/engine.rs`. By the user's request (their brief of 2026-10-09): the physics decides what the engine does, and the sound decides what that sounds like. Each physics step `EngineConfig::step` moves a rotating mass (`Engine::rpm`). It is a pure function over plain data. The fuel makes torque on a six-point curve. A mechanical friction is blended in as the fuel is lifted: the engine braking off the throttle. A turbo's boost follows the speed and the fuel with a lag (0.9 s up, 0.4 s down), and scales a quarter of the torque. Two governors set the fuel, as a diesel's do. The idle governor adds fuel as the speed droops under 770 rpm. The top governor cuts it over the 250 rpm under 3200. With the clutch open, an all-speed governor makes the pedal set the speed. The clutch works in the mode `gearbox::ClutchMode` gives. It transmits at most its capacity: 1.3 of the peak torque, ramped up from 0 over 0.25 s after every change of gear. Within the capacity the engine is locked to the wheels' speed (a lock test, not a stiff spring). Over it the clutch slips. The converter also slips under `launch_rpm`, and can never drag the engine under idle. The driver's clutch stalls the engine under 500 rpm. A stalled engine starts again at idle when the throttle is let go or the gear is changed (no starter yet). The model is calibrated to `TruckConfig::engine_force` and `top_speed`. They stay the tuning knobs that `opponents` and the garage rewrite. The peak torque makes the locked first-gear pull at the curve's best speed `engine_force` per wheel (unit test: within 1%). Second gear pushes 0.54 of first. Each gear's ratio meets the governed speed at its `GEAR_TOPS` share of `top_speed`. The inertia comes from a 0.25 s free rev: 6.2 kg·m² for the built-in truck. That is a heavy flywheel, the price of game-scale torque with a believable free rev. `drive` (its `wheel_force`), the dashboard, the speedometer and `sound` read `Engine`. A truck is spawned with its engine at its own idle (`Engine::at_idle`). Simplified: the engine is coupled to the chassis speed through the gear, not to each wheel. So wheelspin and wheels in the air do not yet rev it (the user's stage 6: wheel rotation and the differential). There are no tire or terrain sounds (stage 7), no brake model (stage 9), no starter or start-up, and no damage. Two changes against the old throttle formula are intended. **Measured** headless on 2026-10-09 with the built-in truck on flat ground. Engine braking in gear: lifting at 15 m/s in first slows the truck 3.1 m/s², 2.9 times the rolling resistance alone; at 30 m/s in third 3.5 m/s², 1.5 times. `friction.1` is the knob. The flat-ground balance speed: 51.4 m/s in third at 2741 rpm after 30 s at full throttle, against about 45.8 m/s from the old formula for the same conditions (+12%). That is within the 15% the design allowed, so the curve's top point stays at (3200, 0.35); 0.2 there would give about 49 m/s. Also **measured** there: the idle settles at 753 rpm with fueling 0.14 (a 17 rpm droop, the proportional idle governor's equilibrium). The launch has the converter slipping at 1282 to 1294 rpm (clutch 0.5, load 0.65), and locking at about 9 m/s. At each change of gear the rpm comes down over about 0.45 s, not over the 0.25 s ramp: a 0.1 s lift, then the clutch ramp, with a flare of about 150 rpm as the fuel returns and the load at the capacity for 0.15 s. The capacity exceeds the engine's torque only late in the ramp. While the load is at the capacity, the clutch dumps the flywheel into the wheels. After the 1 to 2 change at full throttle this asks each wheel for 22.1 kN for about 0.1 s (**measured** on 2026-10-09 in a scratch simulation of the engine on a point mass), against a grip of about 14.7 kN per wheel (1.1 of the built-in truck's weight over four wheels). So `drive`'s `slipping` rule fires and the tires spin at each such change. After 2 to 3 it is 14.4 kN, just under the grip. The old formula pushed nothing for 0.1 s and then its normal force. Whether the surge is wanted is to be judged by driving. Limiting the transmitted torque to about the engine's own during the ramp would remove it. A manual third from a standstill stalls 0.04 s after the throttle goes down, and runs again at once when it is released. The rpm never passed 2995 on level ground, and no figure was NaN. Not yet driven by the user; judged by the headless probe only. |
| Truck paint and liveries chosen in the garage | Open | Game | The garage sets handling, not paint. |

### Racing

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Circuit race: laps, gates in order, timing | Done | File: [situation.md](formats/situation.md) | |
| Starting grid of eight | Done | File: [situation.md](formats/situation.md) | |
| Back to the last checkpoint | Done | | Key C, and the computer's trucks when stuck. How MTM2 does it (for example a helicopter) is not confirmed. |
| Race position | Done | | |
| Computer trucks, up to seven | Done | | First pass, not tuned to be fast. They cut no corner past their next checkpoint and steer through its gate; near a bend they steer at a nearer point of their line, so as not to cut its inside. |
| Computer trucks' speed hints from the track (`cspeed`, `ctype`, `cTrackWidth`) | Open | File: [situation.md](formats/situation.md) | |
| Start sequence (countdown) | Own | Game | 3, 2, 1, GO, with every truck held on the grid (`truck::Held`), and again on a restart. How MTM2 starts a race is not confirmed. Not yet driven. |
| Finish and results screen | Own | Game | Once the player finishes: every truck's place, race time and best lap, kept up to date as the rest finish. The computer then drives the player's truck on, and the camera changes to a random view every 7 seconds. Enter races again. What MTM2's results show is not confirmed. |
| Pause screen | Own | Game | Esc (or a gamepad's Start or Select) stops the game and shows four choices: continue, restart the race (built again from nothing, as GO builds it), save a screenshot (a PNG in `screenshots/`), and cancel the race, which goes back to the front end (or quits a race started from the command line). `src/race/pause.rs`. What MTM2's pause screen offered is not confirmed. Not yet looked at in the game. |
| Race types other than circuit (rally, Summit Rumble, others) | Missing | File: [situation.md](formats/situation.md) | `Track Race Type` is **open**. Which types MTM2 has must be confirmed. The base game's three Summit Rumble arenas load, with 2 gates each, and are raced as circuits. |
| Extra courses on one track (`Extended Course Definitions`) | Done | File: [situation.md](formats/situation.md) | Measured on 46 files and **reference**: MTM2's computer trucks follow `[Course 2]`, often a shortcut (Crazy '98, Sidewinder Canyon). Read into `TrackData::other_courses`; the difficulty chooses which course the drivers follow. What MTM2 does on the 13 files with no `[Course 2]` is **open**; the game's drivers follow the main course there. |
| Championship or season across several tracks | Missing | Game | Confirm what MTM2 has. |
| Difficulty of the computer's drivers | Partial | Own | The user asked for easy, normal and hard; what MTM2 offers is not confirmed. The race screen's DIFFICULTY slider under OPPONENTS (keys `[` and `]`), and `--difficulty=`, set `opponents::OpponentsSettings`. Easy follows the main course with 80 % of the engine and a quarter of the catch-up power; normal `[Course 1]` with 90 % and half; hard `[Course 2]`, MTM2's own line with its shortcuts, with all of both. The extra courses' left-out corners are put back as curves, so the drivers keep to the middle of the road (situation.md, "Corners of the extra courses"); joined straight, hard was 36 put-backs and 86.1 s on Crazy '98. Hard's put-backs on Sidewinder were trucks too fast down into the canyons' U-turns: full braking downhill gives far less than on the level (about 6 m/s² level, 2.7 to 5.3 at 18 % down, 0.8 to 3.7 at 24 %), so drivers count on less braking downhill (`DOWNHILL_BRAKING`). A driver that is behind takes bends a little faster and brakes a little later (`CHASING_CORNER`, `CHASING_BRAKE`); the user turned it off and then back on. Measured headless, seven trucks for 4 minutes (put-backs, best lap): Crazy '98 hard 19 and 53.5 s, normal 4 and 57.7 s, easy 0 and 61.1 s; Sidewinder Canyon hard 9 and 146.4 s, normal 3 and 169.4 s, easy 17 and 222.6 s. Repeated runs give 12 to 19 put-backs on Crazy '98 hard and 7 to 16 on Alpine normal: compare several runs. On Crazy '98 hard the trucks hit a fixed loader inside the bend after checkpoint 4 (the course passes 5.8 m from it), so drivers now steer round fixed solid scenery on the road as round a stopped truck (`Obstacle`), looking 3 s ahead, and slow for nothing in front of them; ramps are driven over. On the courses of 10 base game tracks and the community tracks, only two fixed objects stand on a course: Scrapyard Run's `GYCOLM.BIN` and Sidewinder Canyon's `SN4CHK2.BIN`. Not yet measured or driven since. |

### Presentation

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Chase camera | Done | | |
| Other camera views (in the cab, bumper, far) | Partial | Game | V changes to the cockpit and back; Q, E and B (or the right stick) look left, right and back from either view (`src/camera/views.rs`). A truck with a dashboard (every base truck) shows it in the cockpit, with the eye tipped down a little (`src/camera/dashboard.rs`); the built-in truck shows the road alone. MTM2's list of views is not confirmed. Not yet driven. |
| Speedometer, lap and race readout, compass to the next checkpoint | Done | | |
| Map of the track in the race | Done | Game | M (`keys::Control::Map`) shows and hides it in the bottom right corner (`src/race/map.rs`): the front end's picture (`track::map_image`), a red dot for the player and a blue dot with its place for each of the computer's trucks. Hidden at first, kept from race to race. Whether MTM2 had such a map is not confirmed. Not yet seen in the game. |
| Weather | Own | | Clear is the default, as MTM2 was. The rest is the game's own. Snow freezes the water. |
| Time of day: day, dusk, night | Own | File: [level.md](formats/level.md) | An option, and F8 in a race. MTM2 had dusk and night skies (**reference**); how it lit the ground under them is not measured. |
| Dirt and dust | Own | | Only on loose ground (`track::TrackData::loose_at`). |
| Audio: engine | Partial | File: [sound.md](formats/sound.md) | `src/sound/`. The base game's three engine loops, found in `SOUND.POD` (**measured**), one set per truck. The gear chooses the loop, as the user describes MTM2: M1 in first or reverse pulling away, M2 in the other gears, idle in neutral and standing. The engine model's speed (`src/truck/engine.rs`) sets the pitch, and the throttle the loudness. How MTM2 pitched them is **open**. The start-up, the changes between loops, the gear changes and the reversing beep are not played. A synthesized diesel (a Cummins 5.9 inline-six turbo, pure DSP, driven by the engine model over a lock-free bridge) was built from 2026-10-09 and removed on 2026-10-10 by the user's request, to stay with the base game's sound for now; its two files are kept outside the slice in the scratch directory, not in git. What it taught stays: the audio thread asks RTKit for real-time priority (cpal's `audio_thread_priority` feature, with D-Bus built from source), because at opt-level 0 the audio thread ran dry on a Bluetooth sink (rodio logged `Buffer underrun/overrun`); its effect is not yet judged. |
| Audio: tires, impacts | Out of scope | File: [sound.md](formats/sound.md) | `SOUND.POD` has skids, wheel spin, landings and crashes, known by name only. |
| Audio: music | Out of scope | File: [level.md](formats/level.md) | |
| Audio: commentator | Out of scope | Game | Confirm that MTM2 has one. |

### Front end

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Choose truck and track, with previews | Done | | |
| Garage: set up the truck | Done | | The dials are the game's own. The computer's trucks are set up at random for each race, except for grip, which stays centred. |
| Laps and number of opponents | Done | | |
| Options screen, remembered between runs | Done | | |
| Exit the game | Done | | EXIT, at the right of the tabs. |
| Loading screen | Own | | GO covers the window with a loading screen (`src/ui/loading.rs`): the track, the truck, the laps, the opponents, and a line that says what is being done; nothing on it moves. It is drawn for 3 frames, then the track and trucks are loaded on the main thread and the race is built. Measured headless with 8 community trucks: 0.2 to 0.4 s a truck, up to 1.5 s for a track, and 1.9 to 2.5 s to build the race. The screen stays up until the race's frames are quick again, with the clock stopped. A restart from the pause dialog shows none. |
| Keyboard and gamepad | Done | | |
| Every key can be bound again | Own | | The front end's CONTROLS page, in sections: driving, race, camera and game (`src/keys.rs`). The arrow keys always drive as well. Gamepad buttons are fixed. |
| Force feedback | Missing | Game | |
| Track and truck names from the POD | Missing | File: [level.md](formats/level.md) | The front end shows file names. The `.LVL` names a description file, which is not read. |
