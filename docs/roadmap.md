# Roadmap

This document says what to work on next to make the game a full replacement for
*Monster Truck Madness 2* (MTM2). It also lists every feature and its status.

There are two goals, and each stage serves one or both:

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
| 6 | Scenery that moves, and the rest of the look: knock-over objects, sky, animated textures | Done, but not yet looked at in the game. Stage 11 finishes it. |
| 7 | Base game archives: mount several PODs at once | Done |
| 8 | Trucks: `.TRK`, body, tires, axles, wheel positions | Done, for trucks that carry their own parts |
| 9 | Race flow: front end and garage, start sequence, finish and results, cockpit and look-round views | Done, but not yet driven. The pause screen is missing. Stage 11 finishes it. |
| 10 | Computer trucks: a driver that follows the course | Done, first pass |

## Next: stages 11 to 16

Many items below are **open**: we do not know what MTM2 does. For each one, first find
the fact in a real file or a cited reference, and record it in [formats/](formats/README.md).
Build it only after that. For an item marked **Game**, first confirm what MTM2 does, and
record it here. Do not invent the details (see "POD compatibility rules" in `AGENTS.md`).

### Stage 11: finish what is built

Goal: what the game has now works, and the user has seen it work.

| Item | Status |
| --- | --- |
| Pause screen. Today Esc leaves the race. | Missing |
| Objects of type 8 turn to face the camera. | Partial |
| Look at these in the game, and record what you find: animated models, animated textures, sky, truck lights, dashboard, camera views, start sequence, results screen, frozen water, truck weight | Not yet looked at |

Done when: the pause screen works, type 8 objects face the camera, and each feature in the
list is driven or looked at, with its row updated.

### Stage 12: the rest of the track files

Goal: the game uses all the data that a track POD holds for the look and the race.
POD compatibility.

| Item | Status |
| --- | --- |
| Track and truck names from the POD, in the front end: read the description file that the `.LVL` names | Missing |
| Fog from the track: the fog file that the `.LVL` names | Missing. Format **open**. |
| Lighting from the track: sun direction, shadow strength, lighting table (`.LTE`) | Missing. Only **reference**. |
| Box types 0, 1 and 11: solid or not | **Open** |
| Ramps, cylinders and the stadium in the track file | **Open** |
| Computer trucks' speed hints: `cspeed`, `ctype`, `cTrackWidth` | **Open** |
| Extra courses on one track (`Extended Course Definitions`) | **Open** |
| `Track Race Type` | **Open**. Needed by stage 15. |

Done when: each item is built, or its format is recorded as **open** after a search of the
references and of real files.

### Stage 13: trucks that differ

Goal: each truck is what its POD makes it. POD compatibility.

| Item | Status |
| --- | --- |
| Handling per truck: where MTM2 keeps mass, engine, springs and grip, if they vary | **Open**. The `.TRK` holds none. |
| What MTM2 does with scrape points | **Open**. Today they make the collider. |
| What each light type means (for example, brake lights only when braking) | **Open** |
| The rest of the dashboard: mirror, shifter, shift light | **Open** |

Done when: each item is built, or recorded as **open** after a search.

### Stage 14: ground types

Goal: the ground acts as it looks. Mud, sand and water change how a truck drives, as the
`.TTY` says.

| Item | Status |
| --- | --- |
| Ground types other than ice: speed and grip | **Open**. Only their look is known. |

### Stage 15: the rest of the game

Goal: feature parity. Confirm each item in MTM2 before you build it (**Game**).

| Item | Status |
| --- | --- |
| Race types other than circuit (rally, Summit Rumble, others) | Missing. Needs `Track Race Type` from stage 12. |
| Championship or season across several tracks | Missing |
| Difficulty of the computer's drivers | Missing |
| Map of the track in the race | Missing. The front end paints one already. |
| Other camera views (bumper, far) | Partial |
| Damage | **Open** |
| Gears: automatic and manual | **Open** |
| Force feedback | Missing |

### Stage 16: rare files

Goal: POD compatibility for files that few or no known PODs use. Do this last.

| Item | Status |
| --- | --- |
| MTM1 tracks | **Open** |
| Caves and second layers (`.RA2` to `.RA5`, `.CL1`, `.CL2`) | **Open**. No MTM2 track is known to use them. |
| Pitch and roll of placed objects | **Open**. No object examined has them. |

## Out of scope for now

These are part of MTM2 but are not wanted until the user asks for them:

- Audio: every sound and all music (the rows below marked Out of scope).
- Multiplayer (network play).
- Replays.
- A track editor. Community tracks were made with other tools, such as Traxx.

## Feature status

Every feature, and how far it is done. The stages above are made from these rows.

| Status | Meaning |
| --- | --- |
| **Done** | Works, for every POD examined so far. |
| **Partial** | Works in part. The row says what is missing. |
| **Missing** | Not started. |
| **Open** | We do not know what MTM2 does. Find out before you build it. |
| **Own** | The game has it, but the values are the game's own, not measured from MTM2. |
| **Out of scope** | Not wanted until the user asks for it. |

The **Evidence** column says how we know that MTM2 has the feature:

- **File**: a POD holds data for it. The link goes to the format document.
- **Game**: from how MTM2 plays, not from its files. Nobody has checked these against the
  real game for this project. **Confirm each one before you build it**, and record what you
  find here.

### Content: loading PODs

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Read a `.POD` archive | Done | File: [pod.md](formats/pod.md) | |
| Load a track POD from the command line or the front end | Done | | |
| Load a truck POD | Done | File: [truck.md](formats/truck.md) | |
| The base game's own tracks and trucks in the front end's lists | Done | File: [pod.md](formats/pod.md) | 15 tracks in 13 archives and 20 trucks in `TRUCK2.POD`. All load. Not yet on the command line, which takes archive paths. |
| Community Patch 3 true-colour textures (PNG, TGA) | Done | File: [textures.md](formats/textures.md) | `src/hd_texture.rs` |
| Mount the base game's archives behind a track or truck | Done | File: [pod.md](formats/pod.md) | `src/base_game.rs`. Only the archives that hold a borrowed file are read. What MTM2 does with the order in `POD.INI` is **open**; no two base archives disagree on a file. |
| Files that a track or truck names but does not contain | Done | File: [pod.md](formats/pod.md) | Found in the base archives: all of them, for the 12 tracks and 12 trucks examined, except one model that Critic names and no archive holds. |
| Truck textures with no palette beside them | Done | File: [truck.md](formats/truck.md) | Reference: MTM2 uses `METALCR2.ACT` from its own archives. So does the game. |
| MTM1 tracks | Open | File: [level.md](formats/level.md) | The first line of a `.LVL` may tell MTM1 from MTM2. Its values are open. |

### The track world

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Terrain heights | Done | File: [terrain.md](formats/terrain.md) | |
| Ground textures, laid per cell | Done | File: [textures.md](formats/textures.md) | Whether the whole picture is mirrored is **open**. |
| Bridges and tunnel roofs (ground boxes) | Done | File: [ground_boxes.md](formats/ground_boxes.md) | How textures lie on their sides and bottoms is **open**. |
| Caves and second layers (`.RA2` to `.RA5`, `.CL1`, `.CL2`) | Open | File: [ground_boxes.md](formats/ground_boxes.md) | No MTM2 track is known to use them. |
| Scenery models, fixed and solid | Done | File: [model.md](formats/model.md) | |
| Which box types are solid | Partial | File: [situation.md](formats/situation.md) | Types 0, 1 and 11 are **open**. Everything except 6, 7 and 8 is solid. |
| Objects with a mass that can be knocked over | Done | File: [model.md](formats/model.md) | 91 of Alpine's 323 objects have a mass. Moving objects are done too. See `src/scenery/motion.rs`. |
| Animated models (Order and Jump records, animation control files) | Own | File: [model.md](formats/model.md) | Stage 6. Order and Jump are stepped over (Alpine's helicopter reads every face). Animation control files (`REX.BIN` in Crazy '98, `PUMPJACK.BIN` in Tinhorn Junction, `OP88ANIM.BIN` in Critic) are read as **reference** and **measured**; how they move is the game's own: in a straight line from frame to frame, round and round, in `src/scenery/animation.rs`. How MTM2 moved them, and what their rate means, is **open**. Solid as the first frame. Not yet looked at in the game. |
| Animated textures | Own | File: [model.md](formats/model.md) | Stage 6. The frames are read (**reference** and **measured**); how they are stepped through is the game's own, with the rate read as 16.16 seconds per frame, which no file shows (**open**). Scenery steps through a cycle's tiles (`track/tiles.wgsl`), trucks switch their material's texture (`truck/looks.rs`). The backdrop shows the first frame. Not yet looked at in the game. |
| Objects that always face the camera (type 8) | Partial | File: reference | Not solid; not yet turned to the camera. |
| Pitch and roll of placed objects | Open | File: [model.md](formats/model.md) | No object examined has them. |
| Backdrop round the horizon | Done | File: [situation.md](formats/situation.md) | `backdropType` is **open** (0 on every track). |
| Sky | Done | File: [level.md](formats/level.md) | `src/sky.rs`. The track's own picture in clear weather by day, the base game's cloudy one when overcast by day, its dusk one at a clear dusk, its night one at night; none at an overcast dusk or in fog, rain, a storm or snow. How MTM2 laid the picture on the sky is **open**: the dome is the game's own. Not yet looked at in the game. |
| Fog, from the track | Missing | File: [level.md](formats/level.md), [terrain.md](formats/terrain.md) | The `.LVL` names a fog file. Format **open**. The weather's fog is the game's own. |
| Lighting: sun direction, shadow strength, lighting table (`.LTE`) | Missing | File: [level.md](formats/level.md) | Values follow line 18 of the `.LVL`; only reference. The sun is the game's own. |
| Water | Own | File: [level.md](formats/level.md) | The height is measured. How MTM2 drew water and moved trucks in it is not. |
| Frozen water in snow | Own | | In snow the water is a solid sheet of ice at its level, and tires on it grip as on ice. F7 freezes and thaws it in a race. MTM2 has no frozen water that is known. Not yet driven. |
| Ground types (`.TTY`): ice | Own | File: [texture_types.md](formats/texture_types.md) | Only ice is used. `ICE_GRIP` is the game's own. |
| Ground types: the other hundreds (sounds, spray, speed: mud, sand, water) | Open | File: [texture_types.md](formats/texture_types.md) | Only their look is known. |
| Ramps, cylinders and the stadium in the track file | Open | File: [situation.md](formats/situation.md) | |
| Track music | Out of scope | File: [level.md](formats/level.md) | The `.LVL` names it. See Audio. |

### Trucks

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Body, tires, axles, bars, shocks, driveshaft | Done | File: [truck.md](formats/truck.md) | |
| Wheel positions and size | Done | File: [truck.md](formats/truck.md) | |
| Scrape points | Partial | File: [truck.md](formats/truck.md) | Used as the convex hull of the collider. What MTM2 does with them is **open**. |
| Handling per truck (mass, engine, springs, grip) | Open | File: [truck.md](formats/truck.md) | The `.TRK` holds none. Every truck drives like the built-in one. Where MTM2 keeps handling, if it varies, is **open**. |
| Truck weight | Own | | 5 443 kg (12 000 lb), what a real monster truck weighs; MTM2's is not known (the `.TRK` holds none). The forces were scaled with it from the 2 000 kg tuning, so the truck rides and drives as it did. What changes is how it pushes what it meets: scenery with a mass, knocked by its tires, is pushed 2.7 times as hard. Not yet driven. |
| Lights (headlights, brake lights) | Partial | File: [truck.md](formats/truck.md) | Every light is read. Each lamp's picture glows at the lamp (with bloom), each beam is drawn as MTM2's cone and is a spot light, wider and longer than the cone so that it lights the course, all on at dusk, at night, and in rain and storms (`src/truck/lamps.rs`). Every truck lights the road: headlights with no beam (Maximum Destruction) get the base game's usual one, and a truck with no lights (8 of the 12 community trucks examined) gets the light alone, shining out in front. Glows face the camera. What each light type means (brake lights only when braking?) is **open**. The built-in truck's lamps and the brightness are the game's own. Not yet looked at in the game. |
| Truck sounds | Out of scope | File: [truck.md](formats/truck.md) | `Wave File`: three sound names. Not parsed. |
| Dashboard (instrument cluster) | Partial | File: [cockpit.md](formats/cockpit.md) | `Instrument Cluster` names the layout (`POWERBIG.480`, in the base game's `COCKPIT.POD`), which is read with its pictures. In the cockpit view, the picture for the way the player looks is stretched over the window, and the road shows through its palette index 0 (**measured**). Looking ahead: the steering wheel picture for the steering, and a speedometer and a tachometer needle, at the file's angles (**measured**: clockwise from straight up). The tachometer's engine speed is the game's own. Not drawn: the mirror, the shifter, the shift light (**open**). Not yet looked at in the game. |
| Damage | Open | Game | Confirm whether MTM2 has it. |
| Gears: automatic and manual | Open | Game | Confirm what MTM2 offers. |
| Truck paint and liveries chosen in the garage | Open | Game | The garage sets handling, not paint. |

### Racing

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Circuit race: laps, gates in order, timing | Done | File: [situation.md](formats/situation.md) | |
| Starting grid of eight | Done | File: [situation.md](formats/situation.md) | |
| Back to the last checkpoint | Done | | Key C, and the computer's trucks when stuck. How MTM2 does it (for example a helicopter) is not confirmed. |
| Race position | Done | | |
| Computer trucks, up to seven | Done | | First pass. Not tuned to be fast. |
| Computer trucks' speed hints from the track (`cspeed`, `ctype`, `cTrackWidth`) | Open | File: [situation.md](formats/situation.md) | |
| Start sequence (countdown) | Own | Game | 3, 2, 1, GO, with every truck held on the grid (`truck::Held`), and again on a restart. How MTM2 starts a race is not confirmed. Not yet driven. |
| Finish and results screen | Own | Game | Once the player finishes: every truck's place, race time and best lap, kept up to date as the rest finish. Enter races again. What MTM2's results show is not confirmed. |
| Pause screen | Missing | Game | Esc leaves the race today. |
| Race types other than circuit (rally, Summit Rumble, others) | Missing | File: [situation.md](formats/situation.md) | `Track Race Type` is **open**. Which types MTM2 has must be confirmed. The base game's three Summit Rumble arenas load, with 2 gates each, and are raced as circuits. |
| Extra courses on one track (`Extended Course Definitions`) | Open | File: [situation.md](formats/situation.md) | |
| Championship or season across several tracks | Missing | Game | Confirm what MTM2 has. |
| Difficulty of the computer's drivers | Missing | Game | Confirm what MTM2 offers. |

### Presentation

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Chase camera | Done | | |
| Other camera views (in the cab, bumper, far) | Partial | Game | V changes to the cockpit and back; Q, E and B (or the right stick) look left, right and back from either view (`src/camera/views.rs`). A truck with a dashboard (every base truck) shows it in the cockpit, with the eye tipped down a little so that the road is in its window (`src/camera/dashboard.rs`); the built-in truck shows the road alone. MTM2's list of views is not confirmed. Not yet driven. |
| Speedometer, lap and race readout, compass to the next checkpoint | Done | | |
| Map of the track in the race | Missing | Game | The front end paints a track map already. |
| Weather | Own | | Clear is the default, as MTM2 was. The rest is the game's own. Snow freezes the water. |
| Time of day: day, dusk, night | Own | File: [level.md](formats/level.md) | An option, and F8 in a race. MTM2 had dusk and night skies (**reference**); how it lit the ground under them is not measured. |
| Dirt and dust | Own | | |
| Audio: engine, tires, impacts | Out of scope | File: [truck.md](formats/truck.md) | Needs an audio dependency. Sound file formats are **open**. |
| Audio: music | Out of scope | File: [level.md](formats/level.md) | |
| Audio: commentator | Out of scope | Game | Confirm that MTM2 has one. |

### Front end

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Choose truck and track, with previews | Done | | |
| Garage: set up the truck | Done | | The dials are the game's own. |
| Laps and number of opponents | Done | | |
| Options screen, remembered between runs | Done | | |
| Keyboard and gamepad | Done | | |
| Every key can be bound again | Own | | The front end's CONTROLS page, in sections: driving, race, camera and game (`src/keys.rs`). The arrow keys always drive as well. Gamepad buttons are fixed. |
| Force feedback | Missing | Game | |
| Track and truck names from the POD | Missing | File: [level.md](formats/level.md) | The front end shows file names. The `.LVL` names a description file, which is not read. |
