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
| 9 | Race flow: front end and garage, start sequence, finish and results, cockpit and look-round views | Done, but not yet driven. Stage 11 finishes it. |
| 10 | Computer trucks: a driver that follows the course | Done, first pass |

Outside the stages: the physics engine is now Avian in place of Rapier, by the user's
request. Done and measured headless against Rapier, but not yet driven. See
[avian.md](avian.md).

## Next: stages 11 to 16

Many items below are **open**: we do not know what MTM2 does. For each one, first find
the fact in a real file or a cited reference, and record it in [formats/](formats/README.md).
Build it only after that. For an item marked **Game**, first confirm what MTM2 does, and
record it here. Do not invent the details (see "POD compatibility rules" in `AGENTS.md`).

### Stage 11: finish what is built

Goal: what the game has now works, and the user has seen it work.

| Item | Status |
| --- | --- |
| Pause screen | Done, not yet looked at |
| Objects of type 8 turn to face the camera. | Done, not yet looked at |
| Look at these in the game, and record what you find: animated models, animated textures, sky, truck lights, dashboard, camera views, start sequence, results screen, frozen water, truck weight | Not yet looked at |

Done when: the pause screen is looked at, type 8 objects face the camera, and each feature in the
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
| Cylinders and the stadium in the track file | **Open** |
| Computer trucks' speed hints: `cspeed`, `ctype`, `cTrackWidth` | **Open** |
| Extra courses on one track (`Extended Course Definitions`) | Done: read, and followed by difficulty (see the difficulty row). What MTM2 does on a track with no `[Course 2]` is **open**. |
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
| Ground types other than ice: speed and grip | **Open**. What each shows is known. |

### Stage 15: the rest of the game

Goal: feature parity. Confirm each item in MTM2 before you build it (**Game**).

| Item | Status |
| --- | --- |
| Race types other than circuit (rally, Summit Rumble, others) | Missing. Needs `Track Race Type` from stage 12. |
| Championship or season across several tracks | Missing |
| Difficulty of the computer's drivers | Partial, by the user's request: easy, normal and hard. On hard they take the shortcuts and keep to the middle of the road; at full power they are put back more often in tight bends (see the feature row). |
| Map of the track in the race | Done. M shows it; not yet seen in the game. |
| Other camera views (bumper, far) | Partial |
| Damage | **Open** |
| Gears: automatic and manual | Done, by the user's request: five forward gears and a reverse, automatic by default (see the feature row). |
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
| The world repeats beyond the edges of the map | Done | File: [terrain.md](formats/terrain.md), [situation.md](formats/situation.md) | Monte Carlo's course goes across two edges. The course is joined the short way, a truck that goes over an edge is moved across to the other with its camera, and checkpoints, the computer's drivers, the compass and the map measure the short way. The ground, the ground boxes, the fixed scenery and the water are drawn again 1 km past each edge; what MTM2 showed there is **open**. Driven headless across both of Monte Carlo's edges; not yet driven or measured on screen. |
| Ground textures, laid per cell | Done | File: [textures.md](formats/textures.md) | Whether the whole picture is mirrored is **open**. |
| Bridges and tunnel roofs (ground boxes) | Done | File: [ground_boxes.md](formats/ground_boxes.md) | How textures lie on their sides and bottoms is **open**. |
| Caves and second layers (`.RA2` to `.RA5`, `.CL1`, `.CL2`) | Open | File: [ground_boxes.md](formats/ground_boxes.md) | No MTM2 track is known to use them. |
| Scenery models, fixed and solid | Done | File: [model.md](formats/model.md) | |
| Which box types are solid | Partial | File: [situation.md](formats/situation.md) | Types 0, 1 and 11 are **open**. Everything except 6, 7 and 8 is solid. |
| Objects with a mass that can be knocked over | Done | File: [model.md](formats/model.md) | 91 of Alpine's 323 objects have a mass. Moving objects are done too. See `src/scenery/motion.rs`. |
| Animated models (Order and Jump records, animation control files) | Own | File: [model.md](formats/model.md) | Stage 6. Order and Jump are stepped over (Alpine's helicopter reads every face). Animation control files (`REX.BIN` in Crazy '98, `PUMPJACK.BIN` in Tinhorn Junction, `OP88ANIM.BIN` in Critic) are read as **reference** and **measured**; how they move is the game's own: in a straight line from frame to frame, round and round, in `src/scenery/animation.rs`. How MTM2 moved them, and what their rate means, is **open**. Solid as the first frame. Tinhorn Junction's pump jacks stood still when they came into view after the start, because of a fault in Bevy 0.19; `refresh_morphs_in_view` works round it (measured offscreen). Not yet looked at in the game. |
| Animated textures | Own | File: [model.md](formats/model.md) | Stage 6. The frames are read (**reference** and **measured**); how they are stepped through is the game's own, with the rate read as 16.16 seconds per frame, which no file shows (**open**). Scenery steps through a cycle's tiles (`shaders/tiles.wgsl`), trucks switch their material's texture (`truck/looks.rs`). The backdrop shows the first frame. Not yet looked at in the game. |
| Objects that always face the camera (type 8) | Done | File: [situation.md](formats/situation.md) | Not solid. Turned about the vertical to the camera every frame (`src/scenery/facing.rs`). Which side MTM2 shows, and about which point it turns, are **open**. Type 9 (a facing tree with a solid trunk) does not turn yet. Not yet looked at in the game. |
| Pitch and roll of placed objects | Open | File: [model.md](formats/model.md) | No object examined has them. |
| Backdrop round the horizon | Done | File: [situation.md](formats/situation.md) | `backdropType` is **open** (0 on every track). |
| Sky | Done | File: [level.md](formats/level.md) | `src/sky.rs`. The track's own picture in clear weather by day, the base game's cloudy one when overcast by day, its dusk one at a clear dusk, its night one at night; none at an overcast dusk or in fog, rain, a storm or snow. How MTM2 laid the picture on the sky is **open**: the dome is the game's own. Not yet looked at in the game. |
| Fog, from the track | Missing | File: [level.md](formats/level.md), [terrain.md](formats/terrain.md) | The `.LVL` names a fog file. Format **open**. The weather's fog is the game's own. |
| Lighting: sun direction, shadow strength, lighting table (`.LTE`) | Missing | File: [level.md](formats/level.md) | Values follow line 18 of the `.LVL`; only reference. The sun is the game's own. |
| Water | Own | File: [level.md](formats/level.md) | The height is measured. How MTM2 drew water and moved trucks in it is not. The moving water mirrors the scenery that is in the picture (screen-space reflections); `WaterSettings::reflections` turns it off. |
| Frozen water in snow | Own | | In snow the water is a solid sheet of ice at its level, and tires on it grip as on ice. F7 freezes and thaws it in a race. MTM2 has no frozen water that is known. Not yet driven. |
| Ground types (`.TTY`): ice | Own | File: [texture_types.md](formats/texture_types.md) | Only ice is used. `ICE_GRIP` is the game's own. |
| Ground types (`.TTY`): loose ground | Own | File: [texture_types.md](formats/texture_types.md) | The hundreds of the base game's types are measured: 1 road, 2 dirt, 3 water, 4 mud, 5 sand, 6 grass, 7 rocky ground, 10 metal, 12 rock, 14 railway track. Tires throw up dirt only on 2 and 4 to 7, and, on ground the list doesn't name, where it is coloured and not blue (the game's own rule). Not yet driven. |
| Ground types: the other hundreds (sounds, spray, speed: mud, sand, water) | Open | File: [texture_types.md](formats/texture_types.md) | What each shows is known; what MTM2 did with it is not. |
| Ramps with a model (`RAMP.BIN`, `CRURAMP.BIN`) | Done | File: [situation.md](formats/situation.md) | Read from the Ramps section, and placed and made solid as a box with a model is. Measured on Arizona. Not yet driven. |
| Ramps with no model | Done | File: [situation.md](formats/situation.md) | An invisible, fixed, solid wedge. How to read its sizes is measured on Torture Pit; it disagrees with the reference. Sidewinder Canyon has 8, under its skeletons, and Torture Pit 1. Not yet driven. |
| Cylinders and the stadium in the track file | Open | File: [situation.md](formats/situation.md) | |
| Track music | Out of scope | File: [level.md](formats/level.md) | The `.LVL` names it. See Audio. |

### Trucks

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Body, tires, axles, bars, shocks, driveshaft | Done | File: [truck.md](formats/truck.md) | |
| Wheel positions and size | Done | File: [truck.md](formats/truck.md) | |
| Scrape points | Partial | File: [truck.md](formats/truck.md) | Used as the convex hull of the collider. What MTM2 does with them is **open**. |
| Handling per truck (mass, engine, springs, grip) | Open | File: [truck.md](formats/truck.md) | The `.TRK` holds none. Every truck drives like the built-in one. Where MTM2 keeps handling, if it varies, is **open**. |
| Truck weight | Own | | 5 443 kg (12 000 lb), what a real monster truck weighs; MTM2's is not known (the `.TRK` holds none). The forces were scaled with it from the 2 000 kg tuning, so the truck rides and drives as it did. What changes is how it pushes what it meets: scenery with a mass, knocked by its tires, is pushed 2.7 times as hard. Not yet driven. |
| Lights (headlights, brake lights) | Partial | File: [truck.md](formats/truck.md) | Every light is read. Each lamp's picture glows at the lamp (with bloom), each beam is drawn as MTM2's cone and is a spot light, wider and longer than the cone so that it lights the course, all on at dusk, at night, and in rain and storms (`src/truck/lamps.rs`). The cone is seen only at night; at other times a beam is only the light it casts. Every truck lights the road: headlights with no beam (Maximum Destruction) get the base game's usual one, and a truck with no lights (8 of the 12 community trucks examined) gets the light alone, shining out in front. Glows face the camera. What each light type means (brake lights only when braking?) is **open**. The built-in truck's lamps and the brightness are the game's own. Not yet looked at in the game. |
| Truck sounds | Out of scope | File: [truck.md](formats/truck.md) | `Wave File`: three sound names. Not parsed. |
| Dashboard (instrument cluster) | Partial | File: [cockpit.md](formats/cockpit.md) | `Instrument Cluster` names the layout (`POWERBIG.480`, in the base game's `COCKPIT.POD`), which is read with its pictures. In the cockpit view, the picture for the way the player looks is stretched over the window, and the road shows through its palette index 0 (**measured**). Looking ahead: the steering wheel picture for the steering, and a speedometer and a tachometer needle, at the file's angles (**measured**: clockwise from straight up). The tachometer's engine speed is the game's own. Not drawn: the mirror, the shifter, the shift light (**open**). Not yet looked at in the game. |
| Damage | Open | Game | Confirm whether MTM2 has it. |
| Gears: automatic and manual | Own | | `src/truck/gearbox.rs`. Five forward gears, by the user's request, and a reverse that goes in by itself. Automatic by default; manual is an option (OPTIONS > Gears, `--manual`) for the player's truck only. Only first, second and reverse pull away from a standstill: the throttle in a higher gear at low engine speed stalls the engine until the throttle is let go or the gear is changed. The gear is shown beside the speed, and the tachometer reads the engine speed in the gear. The gear ratios and shift points are the game's own. What MTM2 offers is **open**. The shifter pictures and the shift light of the cockpit are not drawn. Not yet driven. |
| Truck paint and liveries chosen in the garage | Open | Game | The garage sets handling, not paint. |

### Racing

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Circuit race: laps, gates in order, timing | Done | File: [situation.md](formats/situation.md) | |
| Starting grid of eight | Done | File: [situation.md](formats/situation.md) | |
| Back to the last checkpoint | Done | | Key C, and the computer's trucks when stuck. How MTM2 does it (for example a helicopter) is not confirmed. |
| Race position | Done | | |
| Computer trucks, up to seven | Done | | First pass. Not tuned to be fast. They cut no corner past their next checkpoint, and steer through its gate. Near a bend they steer at a nearer point of their line, so that they do not cut across its inside. |
| Computer trucks' speed hints from the track (`cspeed`, `ctype`, `cTrackWidth`) | Open | File: [situation.md](formats/situation.md) | |
| Start sequence (countdown) | Own | Game | 3, 2, 1, GO, with every truck held on the grid (`truck::Held`), and again on a restart. How MTM2 starts a race is not confirmed. Not yet driven. |
| Finish and results screen | Own | Game | Once the player finishes: every truck's place, race time and best lap, kept up to date as the rest finish. The computer then drives the player's truck on, and the camera changes to a random view every 7 seconds, from a random distance to the truck, with a swing round the truck or a fade through black. Enter races again. What MTM2's results show is not confirmed. |
| Pause screen | Own | Game | Esc (or a gamepad's Start or Select) stops the game and shows four choices: continue, restart the race (built again from nothing, as GO builds it: trucks, scenery, countdown, the computer's setups and a random weather), save a screenshot (a PNG in `screenshots/`, without the dialog), and cancel the race, which goes back to the front end (or quits a race started from the command line). `src/race/pause.rs`. What MTM2's pause screen offered is not confirmed. Not yet looked at in the game. |
| Race types other than circuit (rally, Summit Rumble, others) | Missing | File: [situation.md](formats/situation.md) | `Track Race Type` is **open**. Which types MTM2 has must be confirmed. The base game's three Summit Rumble arenas load, with 2 gates each, and are raced as circuits. |
| Extra courses on one track (`Extended Course Definitions`) | Done | File: [situation.md](formats/situation.md) | Measured on 46 files and **reference**: MTM2's computer trucks follow `[Course 2]`, often a shortcut (Crazy '98, Sidewinder Canyon). Read into `TrackData::other_courses`. The difficulty chooses which course the computer's drivers follow. What MTM2 does on the 13 files with no `[Course 2]` is **open**; the game's drivers follow the main course there. |
| Championship or season across several tracks | Missing | Game | Confirm what MTM2 has. |
| Difficulty of the computer's drivers | Partial | Own | The user asked for easy, normal and hard. What MTM2 offers is not yet confirmed. The race screen's DIFFICULTY slider, under OPPONENTS (drawn as the options screen's Quality line; keys `[` and `]`), and `--difficulty=`, set `opponents::OpponentsSettings`. Easy follows the main course with 80 % of the engine and a quarter of the catch-up power; normal follows `[Course 1]` with 90 % and half; hard follows `[Course 2]`, MTM2's own line with its shortcuts, with all of the engine and of the catch-up power, as much as the player's truck. The extra courses have the corners that the file leaves out put back as curves, so that the drivers keep to the middle of the road (situation.md, "Corners of the extra courses"). Measured headless, seven trucks for 4 minutes (put-backs, best lap): Crazy '98 easy 4 and 61.6 s, normal 3 and 57.8 s, hard 12 and 53.4 s; Sidewinder Canyon 17 and 197.3 s, 4 and 170.1 s, 27 and 172.8 s. With the corners joined straight, hard was 36 and 86.1 s on Crazy '98. Hard's put-backs on Sidewinder were trucks too fast down into the canyons' U-turns: braking fully downhill gives far less than on the level (measured: about 6 m/s² level, 2.7 to 5.3 at 18 % down, 0.8 to 3.7 at 24 %). Drivers now count on less braking downhill (`DOWNHILL_BRAKING`). A driver that is behind still takes bends a little faster and brakes a little later (`CHASING_CORNER`, `CHASING_BRAKE`): the user turned it off and then back on. After that, Sidewinder hard 9 and 146.4 s, normal 3 and 169.4 s, easy 17 and 222.6 s; Crazy '98 hard 19 and 53.5 s, normal 4 and 57.7 s, easy 0 and 61.1 s. The same run repeated gives 12 to 19 put-backs on Crazy '98 hard, and 7 to 16 on Alpine normal: compare figures from several runs. On Crazy '98 hard, the trucks hit a fixed loader at the inside of the bend after checkpoint 4: the course passes 5.8 m from it, and a driver's line and the corner it cuts take up the rest. Drivers now steer round fixed solid scenery on the road or reaching into it, as round a stopped truck (`Obstacle`), looking 3 s ahead for it, and slow for nothing in front of them, truck or scenery; ramps are driven over. On the courses of 10 base game tracks and the community tracks, only two fixed objects stand on a course: Scrapyard Run's column `GYCOLM.BIN` and Sidewinder Canyon's checkpoint pillar `SN4CHK2.BIN`. Not yet measured or driven. |

### Presentation

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Chase camera | Done | | |
| Other camera views (in the cab, bumper, far) | Partial | Game | V changes to the cockpit and back; Q, E and B (or the right stick) look left, right and back from either view (`src/camera/views.rs`). A truck with a dashboard (every base truck) shows it in the cockpit, with the eye tipped down a little so that the road is in its window (`src/camera/dashboard.rs`); the built-in truck shows the road alone. MTM2's list of views is not confirmed. Not yet driven. |
| Speedometer, lap and race readout, compass to the next checkpoint | Done | | |
| Map of the track in the race | Done | Game | M (`keys::Control::Map`) shows and hides it in the bottom right corner (`src/race/map.rs`): the front end's picture (`track::map_image`), with a red dot for the player and a blue dot for each of the computer's trucks, with its place in the race in it, counted as the race readout counts the player's. Hidden at first, and kept as it is from race to race. Whether MTM2 had such a map is not confirmed. Not yet seen in the game. |
| Weather | Own | | Clear is the default, as MTM2 was. The rest is the game's own. Snow freezes the water. |
| Time of day: day, dusk, night | Own | File: [level.md](formats/level.md) | An option, and F8 in a race. MTM2 had dusk and night skies (**reference**); how it lit the ground under them is not measured. |
| Dirt and dust | Own | | Only on loose ground (`track::TrackData::loose_at`). |
| Audio: engine, tires, impacts | Out of scope | File: [truck.md](formats/truck.md) | Needs an audio dependency. Sound file formats are **open**. |
| Audio: music | Out of scope | File: [level.md](formats/level.md) | |
| Audio: commentator | Out of scope | Game | Confirm that MTM2 has one. |

### Front end

| Feature | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Choose truck and track, with previews | Done | | |
| Garage: set up the truck | Done | | The dials are the game's own. The computer's trucks are set up at random for each race, except for grip, which stays centred. |
| Laps and number of opponents | Done | | |
| Options screen, remembered between runs | Done | | |
| Exit the game | Done | | EXIT, at the right of the tabs, closes the game. |
| Loading screen | Own | | GO covers the window with a loading screen (`src/ui/loading.rs`): the track, the truck, the laps and the opponents, and a line that says what is being done. Only the text changes from stage to stage, and nothing on it moves. GO closes the front end as the screen comes up. The screen is drawn for 3 frames, then the track and trucks are loaded on the main thread, and the race is built. Measured headless with 8 community trucks, loading took 0.2 to 0.4 s a truck and up to 1.5 s for a track, and building the race 1.9 to 2.5 s. The screen stays up until the race's frames are quick again, with the clock stopped. A restart from the pause dialog shows no loading screen. |
| Keyboard and gamepad | Done | | |
| Every key can be bound again | Own | | The front end's CONTROLS page, in sections: driving, race, camera and game (`src/keys.rs`). The arrow keys always drive as well. Gamepad buttons are fixed. |
| Force feedback | Missing | Game | |
| Track and truck names from the POD | Missing | File: [level.md](formats/level.md) | The front end shows file names. The `.LVL` names a description file, which is not read. |
