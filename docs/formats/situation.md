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
| `ipos` | Vehicles, Boxes | Position in feet: x, height, z |
| `theta,phi,psi` | Vehicles, Boxes | Pitch, roll and heading in radians. At `psi` = 0 a truck faces +Z |
| `model` | Boxes | 3D model file (.BIN) |
| `length,width,height` | Boxes | Replaces `model` for an invisible box, in feet |
| `type,flags` | Boxes | See below |
| `mass` | Boxes | Mass in slugs (pounds divided by g). 0 is a box that nothing can move. See "Mass" below |
| `bvel` | Boxes | Velocity of a moving box (type 10), in feet per second, along the world's axes. See "Moving boxes" below |
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
  and `tightcorners.pod` (`tests/pod_real_tracks.rs`): every model is a flat picture of a
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
  20 lb for a sign, 10 000 lb for a guard rail, 300 000 lb for a lorry.
  `tests/pod_real_tracks.rs` holds this. JSTrackViewer finds the same on the stock tracks.
- Tracks made with other editors write round numbers (`10.0`, `20.0`, `100.0`). The game
  reads them as slugs too.
- The game converts slugs to kilograms (1 slug = 14.594 kg). The masses assume MTM2's
  truck. How heavy MTM2's truck is, is **open** (JSTrackViewer uses 10 000 lb).

## Moving boxes

**Measured** on `MonteCarlo_PZ.pod` (11 boxes) and `rute756jam.pod` (15 boxes):

- `bvel` is along the world's axes, not the box's own. Monte Carlo has two trains. Each
  lies in a line along its own `bvel`, with its locomotive in front. One train has a
  heading of 90 degrees and a `bvel` along +Z; read in the box's own axes, it would go
  sideways. `tests/pod_real_tracks.rs` holds this.
- The Y value of `bvel` is 0 on every moving box. The rest are 45 to 61 m/s (150 to
  200 ft/s).
- Every moving box goes along a level line that runs across the whole map. Where the
  ground falls away from the line, a ground box bridges the dip, with its top level with
  the line (up to 20 m above the ground). So a moving box keeps its height, and does not
  follow the ground. `tests/pod_track.rs` holds this.
- Each box starts with its lowest point on the ground.
- What MTM2 does when a moving box gets to the edge of the map is **open**. The line runs
  across the whole map, and the map repeats, so the game brings the box back on at the
  other side. JSTrackViewer does the same, and also says that it does not know.

## Positions off the map

A box's `ipos` can be off the map: below 0, or beyond the map's width (256 cells of 32
ft, 8192 ft, on every base track). Such a box is where it would be a map's width over,
because the world repeats. **Measured** on the base game's 15 tracks
(`tests/base_game.rs` holds this):

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

## Checkpoints

- The last checkpoint in the file is the finish line. **Measured** on both tracks: in
  `MyTrack.pod` the last of 2 is 77 ft in front of the grid, and in `AlpineMtns.pod` the
  last of 10 is 295 ft from pole position while the other nine are all over 1400 ft away.
  (**Reference** treats the first as the start/finish instead.)
- **Reference** warns that a checkpoint's `psi` is not always written with a consistent
  sign. In our two tracks it always is: all 12 checkpoints face the way the nearest
  course segment runs. The game still takes only the line of the gate from `psi` and
  lets the course decide which way along it is forwards, which costs nothing when they
  agree.
- The starting grid has eight places in both tracks, and all eight face the same way.
  **Measured.** Its shape changes from track to track: `MyTrack.pod` has four rows of two
  (30 ft between the two, 30 to 34 ft between rows), and `AlpineMtns.pod` has two rows of four, with 17 to
  21 ft between neighbours. In Alpine the second four vehicles stand about 25 ft further
  along the way the trucks face than the first four. Thus the first vehicle in the file
  is not always the one furthest forward. Why is **open**. The game converts every place
  (`TrackData::grid`), and `tests/pod_track.rs` makes sure that they are clear of each other.
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
  side of a long straight part of it. Why is **open**. `tests/pod_track.rs` holds this.

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
  (`ctype`, `cspeed`, `cTrackWidth`), a box's `p,q,r` and `priority`, ramps, cylinders and
  stadium, and `backdropType`.
- What box types 1 to 5 and 9 mean. The game goes by `mass` alone, whatever the type.
- What MTM2 does with a moving box at the edge of the map (see "Moving boxes").
- `Track Race Type` (2 in our file): presumably circuit, rally or drag.
- The extra courses after `Extended Course Definitions`.
