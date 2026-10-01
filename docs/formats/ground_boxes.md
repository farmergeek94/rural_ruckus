# Ground boxes (.RA0, .RA1, .CL0)

Solid blocks on the grid of the terrain. Tracks use them for bridges, for the roofs of
tunnels and for walls. There is one box on a cell at most.

## Files

The level file does not name these files. They have the name of the heightmap with a
different extension, in the `DATA` folder. **Measured**: all 8 track archives examined
have them, named like that (`DATA\ALPINE.RA0` beside `DATA\ALPINE.RAW`).

| File | Size | Content |
| --- | --- | --- |
| `.RA0` | 1 byte per cell | The height of the bottom of the box |
| `.RA1` | 1 byte per cell | The height of the top of the box. 0 means no box on the cell. |
| `.CL0` | 12 bytes per cell | Six 16-bit values, little-endian: one texture for each face |

- The meaning of the three files and of `.RA1` = 0 is **reference** (JSTrackViewer, which
  calls them "ground boxes"). It agrees with all our files. Where `.RA1` is 0, `.RA0` is 0
  too, and no box has its bottom above its top (**measured**, all 8 archives).
- The cells are in the same row and column order as the heightmap. Heights are in the
  same steps of 2 ft. **Measured**: see below.
- A bottom of 0 occurs. It makes a column that stands on the floor of the world (56 boxes
  in `Critic.pod`, all 8 in `HANGT.POD`).
- The `.CL0` values have the same code as the texture map (terrain.md): texture index in
  bits 0 to 11, then mirror and quarter turns. The index is into the ground texture list
  (.TEX). **Reference**. **Measured** as far as all indices fall inside the texture list.
- `.CL0` has values on cells with no box as well (65 525 of them in `MyTrack.pod`). They are
  ignored.

Box counts: Alpine 502, Critic 392, ROUTE77 222, MonteCarlo 144, rute756jam 24, MyTrack 11,
HANGT 8, tightcorners 0.

## Where a box stands

Box `(column, row)` covers the same ground as the texture map's cell `(column, row)`: from
sample `(column, row)` to sample `(column + 1, row + 1)`, from 32 x column to
32 x (column + 1) ft along X, and the same along Z. It is a block with vertical sides
along X and Z and a flat top and bottom. **Measured**:

- Past checkpoint 6, `AlpineMtns.pod`'s course crosses a ravine 120 ft deep on a deck of
  24 boxes, cells 39 to 41 by 185 to 192, bottom 119 steps, top 120 steps (240 ft). At
  samples 185 and 193 (where the deck ends) the heightmap is exactly 120 steps in every
  column. Thus the deck meets the ground level at both ends. Half a cell further back,
  the deck would stop over a drop of 50 ft. Warning signs stand at both ends of the deck.
- Over all 8 tracks, for box edges that meet the ground within 10 ft of the box's top,
  the part that meets it within 1 ft: 0.36 on the texture map's cells, 0.27 half a cell
  back, 0.21 a whole cell back.
- In `AlpineMtns.pod` the computer trucks' course crosses two groups of boxes, at
  checkpoint 2 and between checkpoints 8 and 9, above ground that is 20 to 50 ft below
  their undersides. The course has no heights, so it does not tell over from under.
  Between checkpoints 8 and 9 the trucks drive **on top** (the user, from the original
  game). That agrees with the boxes: the ramp after checkpoint 8 ends at a cliff whose
  foot (59.4 m) is level with their tops (59.7 m), and along the course's own line many
  of them are solid from the ground to the top, so there is no way through underneath.
  The vehicles that stand under this group (a bulldozer, a fire engine, an RV, a
  snowcat) sit on the ground under its roof. At checkpoint 2 the trucks drive **under**
  (the user, from the original game): for 110 m the course runs on level ground at
  130 ft, under a roof 1.8 m thick whose underside is 26 ft above it, with no box down
  to the ground on the course's line. So a track can have both, and the game must not
  make one rule for all boxes.

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
  same texture as the ground next to the box on 98 boxes, and the sides never do. That
  face 4 is the top is **measured** less strongly: in `MonteCarlo_PZ.pod`,
  `rute756jam.pod` and `Critic.pod`, face 4 is the only face with a texture different from
  the rest, which is what a road on a deck looks like. It agrees with the reference.
- The game lays the top and the bottom like the ground, and a texture on a side upright,
  left to right as seen from outside. How MTM2 lays a texture on a side, and on the
  bottom, is **open**.

## Open

- Whether MTM2 treats a box as solid all the way through, or only its top.
- How textures lie on the sides and the bottom (see above).
- `.RA2` to `.RA5`, `.CL1` and `.CL2`. In the tracks examined, `.RA2` and `.RA3` are 255
  everywhere and `.RA4` and `.RA5` are 0 everywhere. **Reference** says that in
  Hellbender, another game with the same engine, they are a cave under the ground and a
  second layer of boxes in it. Nothing in MTM2 is known to use them.
