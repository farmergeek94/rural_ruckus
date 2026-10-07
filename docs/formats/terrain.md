# Terrain: heightmap (.RAW) and texture map (.CLR)

## Heightmap

A square grid of unsigned bytes with no header: 65536 bytes for the usual 256 x 256.

- Rows run along Z starting at z = 0, and columns along X starting at x = 0. Sample
  (column, row) is the height at exactly (column x 32 ft, row x 32 ft), with no half-cell
  offset. **Measured.**
- One step is 2 ft. **Measured.**
- Each cell is two triangles, and **cells alternate their diagonal like a chessboard**.
  When column + row is even, the cell is split from sample (column, row) to
  (column + 1, row + 1). When it is odd, it is split the other way, from
  (column + 1, row) to (column, row + 1). **Measured.**
- The world is 256 cells of 32 ft, 8192 ft square, and repeats. **Reference.** The game
  closes the far edges with the heights of the near ones, giving 257 x 257 vertices.
  That it repeats is also **measured**: Monte Carlo's course goes across two edges of the
  map (see "A course across the edge of the map" in situation.md). The game makes every
  MTM2 track repeat (`HeightGrid::repeating`).

### How this was measured

Track editors set scenery down on the ground, so every placement of a model should sit
the same height above it (the height of the model's own origin). `AlpineMtns.pod` has 323
objects scattered over steep mountains, which makes that a sensitive test of everything
above at once:

| Hypothesis | Result |
| --- | --- |
| Rows along Z, columns along X | Objects sit a median 10 ft above ground, consistently per model |
| Transposed, or either axis mirrored | 31 to 45 ft, with no consistency |
| Feet per step, fitted by least squares | 1.985 |
| Half-cell offset in either axis | Every offset fits worse than none |
| Bilinear interpolation within a cell | Typical error 0.045 ft |
| Two triangles per cell | Typical error 0.002 ft |
| One diagonal for every cell, either one | Right for about half the placements, wrong by up to 7 ft for the rest |
| Chessboard, even cells split from (column, row) | Right for 116 of the 116 placements where the diagonals differ |

With all of it right, 33 guard rails sit 3.0 ft up to within 0.1 ft, 130 trees of one kind
11.0 ft up to within 0.6 ft. The game's converted ground equals `Heightmap::ground_feet`
at every object, which also shows that the Z flip keeps the chessboard the right way
round.

This shows where the editor that made the track believed the ground to be. That the game
itself agrees is an assumption, but a safe one: a track whose scenery floated or sank on
every slope would not have been released.

`MyTrack.pod` agrees on the scale: the ground under its start line is 60 steps and its
course is written there at a height of exactly 120.0.

## Texture map

Two bytes per terrain cell, little-endian, in the same row and column order as the
heightmap. Cell (column, row) covers the ground between samples (column, row) and
(column + 1, row + 1).

| Bits | Content |
| --- | --- |
| 0 to 11 | Index into the ground texture list (.TEX) |
| 12 | Mirror top to bottom |
| 13 | Mirror left to right |
| 14 to 15 | Quarter turns |

The index bits are **measured**: masking with `0x0fff` gives indices that all fall inside
the texture list, and the cells that differ from the most common texture trace the road.
The mirror and rotation bits are **measured** too: see textures.md for what they mean
and how that was established.

### How rows and columns were fixed

`MyTrack.pod`'s circuit is a rectangle whose western straight runs along x = 3840 ft and
whose southern straight along z = 3809 ft (from the course segments in the .SIT). Roads
are two cells wide. Road textures appear in columns 119 and 120, either side of
3840 / 32 = 120.0, and in rows 118 and 119, either side of 3809 / 32 = 119.03. Transposed,
the columns would have been 118 and 119.

## Open

- Fog and the lighting table. The water height is in level.md.
