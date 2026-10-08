# Track file (.SIT, "situation")

Text, CRLF line endings. It is a saved game state that doubles as the track definition,
so most of it is per-truck race state that a loader can skip.

## Layout

- Line 1 is the level file name (`nutrack.lvl`).
- A value sits on the line after a label line naming it: `ipos`, then
  `4352.605469,135.998047,3948.785156`. Labels may start with a marker character
  (`!`, `@`, `$`, `^`, `&`), as in `!type,flags`. Values are comma-separated.
- Sections open with a `*** Title ***` line: Vehicles, Ramps, Boxes, Cylinders, Top
  Crush, Course, Stadium, Backdrop, in that order. Each starts with a count.
- Other lines of the same shape are **not** sections: every vehicle has `***Lap time***`
  and `*****Checkpoint times*****` blocks.
- A `*** Your Truck (Not used anymore) ***` block precedes the Vehicles section and has
  the same fields as a vehicle. It is not part of the starting grid.

All of the above is **measured** on `MyTrack.pod`.

## The .SI2 variant

Tracks converted from 4x4 Evolution 2 name the file `.SI2` in place of `.SIT`. Their
archives say so (`Evo2->MTM2: Baja Beach`), and Baja Beach carries the converter's
log, which reports "4x4 Evolution 2, SIT v7". **Measured** on `BAJBEACH_MTM2_HD.POD`
and `TDSNAKE.POD`:

- The layout is that of a .SIT: the same sections in the same order, and no label
  that the ten .SIT files we have do not also use.
- Each has exactly 4096 boxes. MTM2's own tracks have a few hundred at most. Nearly all
  of the boxes are type 7 (drive through): vegetation, which the converter's log says
  it made non-solid.
- The grid stands 2 to 6 ft above the ground, as in a .SIT, so the heights agree with
  the heightmap.

**Reference** (JSTrackViewer, `src/worker/pod-format.js`, `findSitEntries`): the
`.SI2` spelling belongs to MTM2 Community Patch 3. It writes `.SI2` in place of `.SIT`
when an archive has no 8-bit (`.RAW` + `.ACT`) textures, only PNG or TGA ones. The
original game looks only for `.SIT`, so it never lists a track that it cannot draw. The
content is the same as a `.SIT`, which agrees with the measurement above.

The game reads a `.SI2` when an archive has no `.SIT`.

## Fields read

| Label | Where | Meaning |
| --- | --- | --- |
| `Race Track Name` | top | Display name |
| `truckFile` | Vehicles | Truck definition (.TRK) for this grid slot, pole position first |
| `ipos` | Vehicles, Ramps, Boxes | Position in feet: x, height, z |
| `theta,phi,psi` | Vehicles, Ramps, Boxes | Pitch, roll and heading in radians. At `psi` = 0 a truck faces +Z |
| `model` | Ramps, Boxes | 3D model file (.BIN) |
| `length,width,height` | Ramps, Boxes | Replaces `model` for an invisible box, in feet |
| `type,flags` | Boxes | See below. A ramp has none |
| `mass` | Ramps, Boxes | Mass in slugs (pounds divided by g). 0 is a box that nothing can move. See "Mass" below |
| `bvel` | Ramps, Boxes | Velocity of a moving box (type 10), in feet per second, along the world's axes. See "Moving boxes" below |
| `cstart`, `cend` | Course | Ends of one straight piece of the computer trucks' route, in feet. The pieces run in lap order with the corners between them left out: Alpine has 79, making a loop of 8.9 km that passes within 11 ft of every checkpoint but the finish (40 ft). The game joins them end to end as the track's course. |

`psi` = 0 facing +Z is **measured**: the grid's front row has the largest z, the finish
line is 77 ft further along +z, and every truck's `psi` is within 0.03 of 2 pi. Values are
not normalised: checkpoints in the same file have `psi` of 15.7 and 18.9.

## Box types

| Type | Meaning | Source |
| --- | --- | --- |
| 0 | Ordinary scenery: 309 of Alpine's 323 objects (trees, guard rails, signs) | **Measured** that it is the common case; whether it is solid is **open** |
| 1 | Four objects in Alpine | **Open** |
| 6 | Checkpoint. Not solid. Checkpoints count in file order. | **Reference**, quoting the Traxx editor's notes; consistent with both files. The model varies: `CKBOXN.BIN` in one, `PEPBAN1.BIN` to `PEPBAN9.BIN` (banners) and `CKBOX.BIN` in the other |
| 7 | "Drive through": scenery with no collision, such as checkpoint banners | **Reference** |
| 8 | "Always face" (the camera). Not solid. See "Objects that face the camera" below | **Reference** and **measured** |
| 9 | "Collide (facing)": faces the camera as type 8 does, with a solid trunk. 2 boxes in `landsbetween.pod`. The game treats it as ordinary solid scenery | **Reference** (JSTrackViewer `src/scene.js`, quoting Traxx's `TrackPODBox.h`) |
| 10 | Moves along its `bvel`: trains in Monte Carlo, traffic in Route 756. Not moved by anything | **Reference** ("moving - use bvel", the Traxx editor's notes, quoted by JSTrackViewer `src/drive/colliders.js`). Every type 10 box seen has mass 0 |
| 11 | Seen only on boxes with `length,width,height`, standing at the feet of a checkpoint banner | **Measured** that they occur together; the meaning is a guess |

## Ramps

The Ramps section comes before the Boxes section. Its count is 0 in most tracks.

- **Measured** on 14 track files (`DEMO.SIT`, `CIRC5.SIT` and the ten `DRAG*.SIT` and
  `DRAG*.SIX` files in Community Patch 3's `GAME.POD`, and `SNAKE.SIT` and `WAR.SIT` on
  the CD): a ramp is laid out as a box, with `ipos`, `theta,phi,psi`, `model` or
  `length,width,height`, `mass`, `bvel` and `p,q,r`. It has no `type,flags`, no
  `priority` and no sound entries. Every ramp seen has mass 0 and `bvel` 0.
- Two kinds occur. Arizona (`DEMO.SIT`) has one ramp with the model `RAMP.BIN`, and each
  circuit and drag file in `GAME.POD` has six with `CRURAMP.BIN`. Sidewinder Canyon
  (`SNAKE.SIT`, 8 ramps) and Torture Pit (`WAR.SIT`, 1) give only `length,width,height`.
- **Reference** (JSTrackViewer `src/worker/sit-parser.js`, quoting the Traxx editor's
  `TrackPODBox.h`): the editor gives a ramp the box type 99, `BOXTYPE_RAMP`. The game uses
  the same number for a ramp (`box_type::RAMP`).
- **Measured** on Arizona: `RAMP.BIN` is a wedge 50 ft square that
  rises from 0 to 15 ft along the model's Z, with its origin at the foot. Placed as a box
  is, its high edge faces a train that stands across the road, 12.6 m short of it, and is
  level with the train's roof (0.2 m below it). The ramp carries trucks over the train.
  So a ramp with a model is placed and drawn as any box with a model is, and is solid.
- A ramp with no model is a wedge. **Reference** (JSTrackViewer `src/scene.js`, from the
  Traxx editor's `ramppoly`): the editor builds it in the box's eight corners, without
  the top of one end, and shows it as a yellow wedge.
- **Measured** on all 9 such ramps: each is the solid shape under a
  model that trucks drive through.
  - Sidewinder Canyon: each of its 8 ramps (38 x 18 x 10 ft, one 38 x 18 x 8) is within
    4 ft of a cattle skeleton, `SKELTN.BIN` (type 7), at the same heading within
    0.06 rad. The skeleton is 56 ft long, 16 ft wide and 11 ft high, and rises towards
    its +Z end. The wedge's top follows it.
  - Torture Pit: its one ramp (82 x 80 x 30 ft) is on the floor of the pit, 3 m from the
    course, with two spikes `LENSPIK2.BIN` (type 7, 30 ft high) standing on it. The tips
    of the spikes lie along its slope.
- How to read the wedge is **measured** on Torture Pit: of 16 readings (whole or half
  sizes, `ipos` at the foot or the middle, rising towards the heading or away from it,
  the length along the heading or across it), only one puts the lip on the far rim of the
  pit (within 2 ft) with the course running up it:
  - `length,width,height` are the whole sizes, in feet. This disagrees with JSTrackViewer,
    which reads a box's sizes as half sizes. Read as half sizes, the skeletons' wedges
    would be 36 ft wide and 20 ft high, twice the skeleton.
  - `ipos` is at the middle of the foot's level: the wedge stands on it.
  - The length lies along the heading (the model's Z), and the slope rises towards the
    heading.
- The game makes such a ramp an invisible, fixed, solid wedge. The file gives it no model
  and no texture, so there is nothing to draw it with. Whether MTM2 draws anything for it
  is **open**.

## Objects on sloping ground

**Measured** on the 28 tracks in the base game's archives (Community Patch 3's `GAME.POD`
with the CD's track archives) and the 12 community tracks in `tracks/`:

- On the base game's tracks, nearly every box stands with its model's lowest point on the
  ground under its origin, as model.md says. No box in them has a pitch or a roll
  (`theta`, `phi`). Four community tracks do: `BAJBEACH_MTM2_HD.POD` (85 boxes),
  `TDSNAKE.POD` (127), `MonteCarlo_PZ.pod` (48) and `ROUTE77.POD` (4). The game turns
  boxes about the vertical only.
- So a model with a broad, flat foot on a slope floats at its downhill corners. On
  Sidewinder Canyon (`SNAKE.SIT`) the checkpoint pillars (`SN4CHK1.BIN` to `SN4CHK5.BIN`,
  5.8 m across and 12.2 m high) float by 0.03 to 0.66 m, and 18 of its 241 objects by more
  than 0.3 m. Trees on steep slopes float most at their lowest branches: 5.4 m in Alaska
  (`AK8PINEC.BIN`, 45.7 m high) and 8.2 m in Graveyard (`DTREE1.BIN`, 15.2 m).
- Some boxes stand above the ground on purpose, with every corner of their foot clear of
  it: the start lights' gantry over the road (`STRTLITE.BIN`, 7.3 m up), bridges
  (`BRIGSIDE.BIN`, 9 to 11 m) and helicopters (`HELI.BIN`, 19 to 25 m).
- Whether MTM2 draws such a foot floating, as its placement says, is **open**.

The game lowers a fixed object that stands on the ground until the corner of its foot
furthest above the ground touches it, by no more than a tenth of the model's height
(`TrackSettings::settle_scenery`, on by default). It does not lower loose or moving
objects, or objects whose whole foot is more than 0.3 m above the ground. How far to
lower each object is worked out once, as the track is loaded (`track/settle.rs`).

## Objects that face the camera

- **Reference** (JSTrackViewer: `src/scene.js` names type 8 `BOXTYPE_NO_COLLIDE_FACING`
  after Traxx's `TrackPODBox.h`; `src/drive/colliders.js` quotes Traxx's `Model Types.txt`:
  8 is "always face"). The viewer quotes Traxx's `TrackPOD.cpp` on types 8 and 9:
  "Facing object? Allow all directions." Type 8 has no collision.
- **Reference**: JSTrackViewer turns such an object about the vertical only, towards the
  camera, and replaces the box's own `psi`. The Traxx editor itself draws it at its `psi`.
  The engine of 4x4 Evolution, by the same makers, turns its "facing" objects about the
  vertical only (JSTrackViewer `src/evo-track-loader.js`).
- **Measured** on 887 type 8 boxes in `ROUTE77.POD`, `landsbetween.pod`, `rute756jam.pod`
  and `tightcorners.pod`: every model is a flat picture of a
  tree, palm or smoke, upright in the model's X and Y (Z within 0.11 ft of 0), with faces
  of type 51 (cutout). Half its faces are wound to be seen from +Z and half from -Z, so it
  is drawn on both sides. Some boxes have a `psi` that is not 0.
- The game turns it about the vertical through the model's origin so that the side seen
  at `psi` = 0 from the track's -z faces the camera. This is JSTrackViewer's choice; which
  side MTM2 shows is **open**, and so is whether the picture is mirrored. The pivot MTM2
  uses is **open**: JSTrackViewer turns it about the middle of the model's bounding box.

## Mass

- **Reference** (the Traxx editor's notes, quoted by JSTrackViewer
  `src/drive/params/mtm2-feel.js`): "0.000000 mass means unmoveable". The game makes a
  solid box with a mass above 0 loose: it stands until something knocks it over or away.
  A box that is not solid (types 6, 7 and 8) never has a mass in the files seen.
- The unit is the slug, pounds divided by g (32.174 ft/s²). **Measured**: all 91 masses in
  `AlpineMtns.pod` become a whole number of pounds, a multiple of 5, when multiplied by g:
  20 lb for a sign, 10 000 lb for a guard rail, 300 000 lb for a lorry. JSTrackViewer
  finds the same on the stock tracks.
- Tracks made with other editors write round numbers (`10.0`, `20.0`, `100.0`). The game
  reads them as slugs too.
- The game converts slugs to kilograms (1 slug = 14.594 kg). The masses assume MTM2's
  truck. How heavy MTM2's truck is, is **open** (JSTrackViewer uses 10 000 lb).

## Moving boxes

**Measured** on `MonteCarlo_PZ.pod` (11 boxes) and `rute756jam.pod` (15 boxes):

- `bvel` is along the world's axes, not the box's own. Monte Carlo has two trains. Each
  lies in a line along its own `bvel`, with its locomotive in front. One train has a
  heading of 90 degrees and a `bvel` along +Z; read in the box's own axes, it would go
  sideways.
- The Y value of `bvel` is 0 on every moving box. The rest are 45 to 61 m/s (150 to
  200 ft/s).
- Every moving box goes along a level line that runs across the whole map. Where the
  ground falls away from the line, a ground box bridges the dip, with its top level with
  the line (up to 20 m above the ground). So a moving box keeps its height, and does not
  follow the ground.
- Each box starts with its lowest point on the ground.
- What MTM2 does when a moving box gets to the edge of the map is **open**. The line runs
  across the whole map, and the map repeats, so the game brings the box back on at the
  other side. JSTrackViewer does the same, and also says that it does not know.

## Positions off the map

A box's `ipos` can be off the map: below 0, or beyond the map's width (256 cells of 32
ft, 8192 ft, on every base track). Such a box is where it would be a map's width over,
because the world repeats. **Measured** on the base game's 15 tracks:

- 5 tracks have boxes off the map, from -4066 to -448 ft: Sidewinder Canyon (202 of 243
  boxes), Tumbleweed Flats (99 of 126), Voodoo Island (146 of 419), The Excavation (81 of
  278) and Torture Pit (48 of 50). Farm Road 29 and Tinhorn Junction have 1 and 2. No
  track has a course point or a grid place off the map.
- Checkpoints: taken as written, 23 checkpoints on those 5 tracks were 1.5 to 3.2 km
  from the course. A map's width over, each one is 0.1 to 18 m from it, and the course
  goes through each in lap order. Clamped to the edge of the map, none of them is.
- Scenery: a map's width over, the median box off the map is 4 to 13 ft above the ground
  there, as the boxes on the same map are (6 to 15 ft). Clamped to the edge of the map, it
  is 32 to 167 ft.

The game moves each such position by whole map widths onto the map. A position on the
map, its far edge included, is not changed.

## A course across the edge of the map

A course can go off one edge of the map and come back on at the opposite edge. **Measured**
on Monte Carlo (`MonteCarlo_PZ.pod`), the one track of the 12 user tracks examined that does
it:

- The course runs west to a point at x = 25.5 ft, and the next point is at x = 8153 ft,
  at almost the same z (4423 and 4419.5 ft). Across the edge at x = 0 they are 64.5 ft
  apart. The long way, across the map, they are 8127.5 ft apart.
- Later the course runs south to z = 8154.5 ft, and the next point is at z = 26.5 ft:
  64 ft apart across the edge at z = 8192.
- The sixth of the 20 checkpoints in the file is on that edge, at x = 0 ft.
- Joined the short way, across the edges, the lap is 9352 m and every checkpoint is
  within 5.1 m of the course, in lap order. Joined straight across the map, the lap was
  14267 m, with two pieces of 2.5 km.

So the world repeats for the course and for the trucks, as it does for positions. The game
joins each pair of course points the short way, across an edge where that is shorter,
and moves a truck that goes over an edge to the other (`truck::RepeatingWorld`). What MTM2
showed beyond the edge is **open**: the game draws the ground, the ground boxes, the fixed
scenery and the water again there.

## Checkpoints

- The last checkpoint in the file is the finish line. **Measured** on both tracks: in
  `MyTrack.pod` the last of 2 is 77 ft in front of the grid, and in `AlpineMtns.pod` the
  last of 10 is 295 ft from pole position while the other nine are all over 1400 ft away.
  (**Reference** treats the first as the start/finish instead.)
- The finish line is just in front of the grid, and the first checkpoint in the file comes
  after it. **Measured** on 33 of the 34 tracks with a course and more than two
  checkpoints (the 11 user tracks and the base game's): the last checkpoint is 9 to 116 m
  from pole position. The exception is Lands Between (`landsbetween.pod`), see below.
- **Reference** warns that a checkpoint's `psi` is not always written with a consistent
  sign. The game takes the line of the gate from `psi` and lets the course decide which way
  along it is forwards: it turns `psi` round where a piece of course near the checkpoint
  runs against it, and none near it runs with it. "Near" is within half the course's width
  (9.75 m) of the nearest piece. **Measured** on 40 tracks, against the way the course
  passes each checkpoint in lap order: a course can pass a checkpoint twice, once each
  way. Lands Between passes its first checkpoint eastwards, as `psi` says, 4.3 m from its
  middle, and westwards 2.7 m from it; its fourth, with `psi`, 7.2 m further than the
  nearest pass; The Tight Corners' fifth, 0.3 m further. Taking the nearest piece alone
  turned all three the wrong way. In the drag arenas Tacoma Dome and Trans World Dome,
  checkpoints 1 to 3 have `psi` the wrong way, and the course runs with it only 15.5 m
  further than the nearest piece.

### Lands Between

**Measured** on `landsbetween.pod`, whose own picture (`UI\LANDSBES.BMP`) calls it "Lands
Between [Beta]":

- Its grid was never set out: eight places in a line 10 ft apart, all at a height of 200
  ft and a heading of 0. On every other track the places are 16.8 ft apart or more. The
  game skips every other place, as it does any place too near another (`PLACE_CLEARANCE`).
- Its last checkpoint, the finish, is 1268 m from pole position and 8854 m round the
  course; the first is 231 m away. Past the finish the course goes off a cliff 107 m (350
  ft) high, which falls between two neighbouring samples of the heightmap, into the sea,
  and 4 km round by sea back to the grid. So the race the game runs, which begins at the
  start line, sends the trucks across country to it first. What MTM2 does on such a track
  is **open**.
- Its ground is islands in a sea 2 ft deep: the heightmap is 0 on two thirds of the map,
  and the water height is 4 (2 ft), where every other track with water has 40 to 388.
- The ground, the scenery and the course agree as terrain.md reads them: 96 % of its 324
  boxes stand 0 to 40 ft above the ground, and the boxes of one model agree to 0.7 ft;
  transposed, mirrored, turned or shifted, 7 to 15 % do. The heights of the course's
  points (`cstart`, `cend`) are the ground's: the median difference is 0.0 ft, as on Monte
  Carlo. Its Course section holds three courses of 43 pieces each, a little different;
  the game reads the first.
- The starting grid has eight places in both tracks, and all eight face the same way.
  **Measured.** Its shape changes from track to track: `MyTrack.pod` has four rows of two
  (30 ft between the two, 30 to 34 ft between rows), and `AlpineMtns.pod` has two rows of four, with 17 to
  21 ft between neighbours. In Alpine the second four vehicles stand about 25 ft further
  along the way the trucks face than the first four. Thus the first vehicle in the file
  is not always the one furthest forward. Why is **open**. The game converts every place
  (`TrackData::grid`). **Measured**: the places are clear of each other on every track we
  have.
- The starting grid need not face the finish line. Alpine's faces -Z while its course
  sets off diagonally, and the finish is ahead and to one side.
- A checkpoint is as wide as its model (see model.md). Where the model isn't in the
  archive, the game assumes 32 ft either side of its centre, which spans a two-cell road.
- A checkpoint can have no model: then it is an invisible box, and its `width` is how wide
  the gate is, from end to end, along the model's X axis. **Measured.** All 18 checkpoints
  of Baja Beach are such boxes, from `2,104,57.57` to `2,198,54.64` ft: thin along the
  way of travel and 104 to 198 ft wide. Under this rule the route of the computer trucks
  goes through 16 of the 18 gates; with 32 ft either side it goes through 10. The same
  rule fits other boxes: in Route 77, a `1,178,7` box lies along a guard rail that is
  192 ft long in X, with the same heading. Its `width` is not a half size (356 ft).
  The two gates that the route does not go through (10 and 14) are 34 to 37 m to one
  side of a long straight part of it. Why is **open**.

## Backdrop

The last section names the models drawn round the horizon. **Measured** on 12 tracks:

```
*** Backdrop ***
backdropType,backdropCount
0,1
backdropModelName
CROKDROP.BIN
```

- The count is the second value after `backdropType,backdropCount`, and that many model
  names follow `backdropModelName`, one to a line. `tground.pod` names two
  (`cs4drop1.bin`, `cs4drop2.bin`), nine name one, and Baja Beach and Snake River write
  `0,0` and no name. Names may be in lower case.
- `backdropType` is 0 on every track. Its meaning is **open**.
- The models are in `MODELS`, and their textures and palettes in `ART`, in 9 of the 11
  archives that name one. MyTrack names `ee4drop.bin` and carries none of it: it is in
  the base game's archives. Monte Carlo carries its model, `TM99DROP.BIN`, but not its
  texture (`DD4DROP.RAW`).
- How the game draws them is in model.md.

## Open

- Everything not listed above, including the Course section's other fields
  (`ctype`, `cspeed`, `cTrackWidth`), a box's `p,q,r` and `priority`, cylinders and
  stadium, and `backdropType`.
- Whether MTM2 draws anything for a ramp with no model (see "Ramps").
- What box types 1 to 5 and 9 mean. The game goes by `mass` alone, whatever the type.
- What MTM2 does with a moving box at the edge of the map (see "Moving boxes").
- `Track Race Type` (2 in our file): 0 on the hill climb practice, 1 on the five drag
  arenas, 2 on 20 circuits, 3 on 11 tracks that include Highlands Rally, and 4 on the three
  Summit Rumble arenas. **Measured** on 40 track files, the user tracks and the base game's.
  So 3 is very likely a rally. How a rally differs from a circuit is **open**: 10 of the
  11 have their grid just behind their last checkpoint, as circuits do.
- The extra courses after `Extended Course Definitions`.
