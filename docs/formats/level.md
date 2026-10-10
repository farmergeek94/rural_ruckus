# Level file (.LVL)

Text, CRLF line endings. A list of the other files that make up a track, identified by
line position, then lighting values. Names are bare file names.

| Line (from 1) | Content | Example |
| --- | --- | --- |
| 1 | A number: `0` in `MyTrack.pod`, `4` in `AlpineMtns.pod`. **Reference**: tells MTM1 levels from MTM2 ones. Its values are **open**. | `0` |
| 2 | Track description text | `nutrack.txt` |
| 3 | Heightmap | `nutrack.raw` |
| 4 | Texture map | `nutrack.clr` |
| 5 | Palette. Need not share the track's name: Alpine uses `metalcr2.act`. | `nutrack.act` |
| 6 | Ground texture list | `nutrack.tex` |
| 7 | `zero.raw` on every level: a 78-byte text file, `Ground Quake Count` 0 and `Box Quake Count` 0. **Measured**. | `zero.raw` |
| 8 | Power-ups (`.PUP`). **Reference** for MTM1's engine only. Missing or empty in every MTM2 archive (**measured**). | `Alaska.pup` |
| 9 | Animated ground textures (`.ANI`). **Reference** (JSTrackViewer, `ani-parser.js`). **Measured**: `AZTEC.ANI` holds `1`, `DWATA1.RAW`, `4,9830`, then 4 frame names. Not read yet. | `Alaska.ani` |
| 10 | Tunnels (`.TDF`). **Reference** for MTM1's engine only. Missing from every archive (**measured**). | `Alaska.tdf` |
| 11 | The sky's picture. See "Sky" below. | `cloudy2.raw` |
| 12 | The sky's palette. | `cloudy2.act` |
| 13, 14 | Objects (`.DEF`) and navigation (`.NAV`). **Reference** for MTM1's engine. Missing from every archive (**measured**): MTM2 uses the `.SIT`. | `Alaska.def` |
| 15 | Music. Native tracks' `.WAV`s are in `MUSIC.POD`; MTM1-derived tracks carry `.MOD`s (**measured**). | `rockx.wav` |
| 16 | Fog table (`.FOG`). Missing for every native track; 4096 bytes on the three whose line 1 is 4 (**measured**). Its distances are **open**. | `Alaska.fog` |
| 17 | Ground light (`DATA\*.LTE`): 458752 bytes, 7 per grid point, on every track (**measured**). **Reference** (JSTrackViewer, `terrain-builder.js`): the first byte is the ground's brightness. The other 6 are **open**. | `Alaska.lte` |
| 18 | The sun's direction, (east, up, north), 16.16 fixed point: every one measured is about 65536 long. **Reference** (JSTrackViewer, `scene.js`, from the Traxx editor). | `-46333,-46333,0` |
| 19 to 22 | Named "shadow intensity", "sun position", "sun intensity" and a fourth value (**reference**, names only). Lines 20 to 22 are the same on all 27 levels measured. Scales **open**. | `32768` |
| After `!waterHeight` | Water height, in half feet. See below. | `252` |

Lines 2 to 6 are **measured** against `MyTrack.pod`: the named files exist with the
expected sizes (heightmap 256 x 256 bytes, texture map twice that, palette 768 bytes).
Lines 7 to 22 are measured on the 12 user tracks and the 15 levels of the base game.
Lines 2 to 6, 11, 12 and the water height are parsed. JSTrackViewer counts lines from 0:
its line 10 is line 11 here.

## Sky

Line 11 names the sky's picture and line 12 its palette (**reference**, JSTrackViewer
`sit-parser.js`; **measured** on 27 levels). All 15 base levels and 9 of the 12 user
tracks name `cloudy2.raw` and `cloudy2.act`. The three whose line 1 is 4 (Alpine, Critic,
Hang Time) name `aliensky.raw` and `earthsky.act`.

- **Size**: a square of palette indices like any texture. `CLOUDY2.RAW` is 256 x 256,
  `ALIENSKY.RAW` 64 x 64. **Measured**.
- **Colours**: not the palette's own entries. Every pixel of the base game's skies is in
  slots 230 to 245, and the sky palettes have colours only at 192 to 207: slot `p` is
  colour `p - 38`. `ALIENSKY` uses slots 244 to 251 of 240 to 255: colour `p - 48`.
  **Measured**: for the base game's four skies, 38 is the only shift that puts every
  pixel on a colour; `ALIENSKY`'s 8 slots fit several shifts, and 48 is the one the
  reference gives. Every native track's palette leaves slots 230 to 245 black, and
  `METALCR2.ACT` leaves 240 to 255 black. **Reference** for the second shift
  (JSTrackViewer, `lvl-parser.js`, for MTM1's engine): the sky palette's colours 192 to
  207 are copied into slots 240 to 255. The parser scores both shifts for each sky
  (`crates/pod/src/sky.rs`); every pixel of every base sky falls in the slots it chooses
  (**measured**).
- **Which way up**: row 0 is the top of the sky and the last row the horizon. The left
  and right edges meet; the top and bottom do not. **Measured**: `CLOUDY2` goes from
  darker blue at the top to a flat pale band at the bottom (its last 32 rows are one
  slot), `DUSKSKY` from dark to orange. Across the left and right edges the pixels differ
  no more than between neighbouring columns; across top and bottom 8 to 90 times more.
  `ALIENSKY` repeats both ways.
- **Weather skies**: `STARTUP.POD` also has `CCLOUDS`, `DUSKSKY` and `NITESKY`, each
  256 x 256 with a palette beside it (**measured**). **Reference** (JSTrackViewer README
  and `scene.js`): MTM2 offers clear, cloudy, dusk and night skies with these pictures.
- **Open**: how MTM2 laid the picture on the sky (a dome or a band, how many times round,
  how high), and whether its weather setting replaced the level's sky. The game's choice
  is in `src/sky.rs`.

## Water height

The line `!waterHeight` is a label. The next line is the height of the water surface. One
water level covers the whole track. The course can go below it.

- The label is not at a fixed line: find it by its text. **Measured** on 6 tracks: it is
  line 23, after the five lighting values.
- A level without the label has no water. **Measured** on 6 tracks. MTM2's arenas write
  `0` for no water. **Reference**
  ([JSTrackViewer](https://github.com/juanputrerasm/JSTrackViewer),
  `src/worker/sit-parser.js`).
- The value is in quarters of a height step. A step is 2 ft, so the value is in half
  feet. **Measured**. JSTrackViewer divides it by 4 to get steps, which agrees.

### How the scale was measured

A track editor does not put the starting grid under water. It puts coral under water and
a raft on it. **Measured** for each scale:

| Value divided by | Result |
| --- | --- |
| 1 (steps) | All the ground of MyTrack and rute756jam is under water. |
| 2 (feet) | The starting grids of MyTrack, rute756jam and Baja Beach are under water. |
| **4 (steps), 2 (feet)** | **All grids are dry. On Baja Beach, all 17 corals are under the water and the raft is 3.8 ft above it.** |
| 8 | 0.3% of Baja Beach is under water. Its corals are above the water. |

The conversion log in `BAJBEACH_MTM2_HD.POD` is a second check. The converter put 4x4
Evolution heights of 60 to 543 ft on steps 0 to 255, at 0.528 steps per foot. Evolution's
water is at 179 ft: (179 - 60) x 0.528 = 62.8 steps. The level file gives 252 / 4 = 63.

In half feet, the water on Baja Beach is at 126 ft, and the lowest point of its course at
127.4 ft. On Snake River Canyon, one point of the course is 1 ft below the water, at a
river.

### Open

- How MTM2's trucks move in water. The game's buoyancy and drag are its own (`src/water.rs`).
- The colour of the water. The level file does not give it.
