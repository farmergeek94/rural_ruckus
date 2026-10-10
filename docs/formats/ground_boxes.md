# Ground boxes (.RA0, .RA1, .CL0)

Solid blocks on the grid of the terrain: bridges, the roofs of tunnels and walls. At most
one box on a cell.

## Files

The level file does not name them. They have the heightmap's name with another
extension, in `DATA`. **Measured**: all 8 track archives examined have them
(`DATA\ALPINE.RA0` beside `DATA\ALPINE.RAW`).

| File | Size | Content |
| --- | --- | --- |
| `.RA0` | 1 byte per cell | The height of the bottom of the box |
| `.RA1` | 1 byte per cell | The height of the top of the box. 0 means no box on the cell. |
| `.CL0` | 12 bytes per cell | Six 16-bit values, little-endian: one texture for each face |

- The meaning of the three files and of `.RA1` = 0 is **reference** (JSTrackViewer, which
  calls them "ground boxes"). **Measured**, all 8 archives: where `.RA1` is 0, `.RA0` is 0
  too, and no bottom is above its top.
- Cells are in the heightmap's row and column order, and heights in its steps of 2 ft.
  **Measured**: see below.
- A bottom of 0 makes a column that stands on the floor of the world (56 boxes in
  `Critic.pod`, all 8 in `HANGT.POD`).
- `.CL0` values use the texture map's code (terrain.md): an index into the ground texture
  list (.TEX) in bits 0 to 11, then mirror and quarter turns. **Reference**. **Measured**
  as far as every index falls inside the list.
- `.CL0` has values on cells with no box too (65 525 in `MyTrack.pod`). They are ignored.

Box counts: Alpine 502, Critic 392, ROUTE77 222, MonteCarlo 144, rute756jam 24, MyTrack 11,
HANGT 8, tightcorners 0.

## Where a box stands

Box `(column, row)` covers the texture map's cell `(column, row)`: from sample
`(column, row)` to `(column + 1, row + 1)`, 32 x column to 32 x (column + 1) ft along X
and the same along Z. It has vertical sides along X and Z and a flat top and bottom.
**Measured**:

- Past checkpoint 6, `AlpineMtns.pod`'s course crosses a ravine 120 ft deep on a deck of
  24 boxes, cells 39 to 41 by 185 to 192, bottom 119 steps, top 120 steps (240 ft). At
  samples 185 and 193, where the deck ends, the heightmap is exactly 120 steps in every
  column, so the deck meets the ground at both ends. Half a cell back, it would stop over
  a drop of 50 ft. Warning signs stand at both ends.
- Over all 8 tracks, of the box edges that meet the ground within 10 ft of the box's top,
  the part within 1 ft is 0.36 on the texture map's cells, 0.27 half a cell back and 0.21
  a whole cell back.
- In `AlpineMtns.pod` the computer trucks' course crosses two groups of boxes, at
  checkpoint 2 and between checkpoints 8 and 9, over ground 20 to 50 ft below their
  undersides. The course has no heights, so it does not tell over from under. Between
  checkpoints 8 and 9 the trucks drive **on top** (the user, from the original game). The
  boxes agree: the ramp after checkpoint 8 ends at a cliff whose foot (59.4 m) is level
  with the tops (59.7 m), and along the course's line many boxes are solid from the
  ground to the top. The vehicles under this group (a bulldozer, a fire engine, an RV, a
  snowcat) sit on the ground under its roof. At checkpoint 2 the trucks drive **under**
  (the user, from the original game): for 110 m the course runs on level ground at 130 ft
  under a roof 1.8 m thick, its underside 26 ft up, with no box down to the ground on
  the course's line. One track has both, so the game must not make one rule for all boxes.

**Measured**: the game's collider holds a truck on the deck.

## Faces

| Face | Side |
| --- | --- |
| 0 | -Z (south) |
| 1 | +Z (north) |
| 2 | +X (east) |
| 3 | -X (west) |
| 4 | Top |
| 5 | Bottom |

- The order of the sides is **reference** (JSTrackViewer).
- Faces 4 and 5 are horizontal. **Measured**: in `ROUTE77.POD`, faces 4 and 5 have the
  texture of the ground beside the box on 98 boxes, and the sides never do. That face 4
  is the top is **measured** less strongly: in `MonteCarlo_PZ.pod`, `rute756jam.pod` and
  `Critic.pod`, face 4 is the only face with a different texture, as a road on a deck
  looks. It agrees with the reference.
- The game lays the top and the bottom like the ground, and a side texture upright, left
  to right as seen from outside. How MTM2 lays a texture on a side or on the bottom is
  **open**.

## Open

- Whether MTM2 treats a box as solid all the way through, or only its top.
- How textures lie on the sides and the bottom (see above).
- `.RA2` to `.RA5`, `.CL1` and `.CL2`. In the tracks examined, `.RA2` and `.RA3` are 255
  everywhere and `.RA4` and `.RA5` are 0 everywhere. **Reference** says that in
  Hellbender, another game with the same engine, they are a cave under the ground and a
  second layer of boxes in it. Nothing in MTM2 is known to use them.
