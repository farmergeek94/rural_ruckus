# POD archive

The first-generation Terminal Reality archive, used by MTM2. Little-endian. There is no
magic number.

| Offset | Size | Content |
| --- | --- | --- |
| 0 | 4 | `u32` number of files |
| 4 | 80 | Comment, NUL-padded text |
| 84 | 40 x count | Directory entries |
| after the directory | | File data |

Each directory entry:

| Offset | Size | Content |
| --- | --- | --- |
| 0 | 32 | Path, NUL-padded, with backslashes, such as `DATA\NUTRACK.RAW` |
| 32 | 4 | `u32` size in bytes |
| 36 | 4 | `u32` offset from the start of the archive |

**Measured** on both archives (20 and 366 entries): the first file begins where the
directory ends, and the files tile the rest of the archive exactly, with no gaps and
nothing after the last one.

Files refer to each other by bare file name in any case (`nutrack.raw` for
`DATA\NUTRACK.RAW`), so lookups ignore case. **Measured.**

Each kind of file has a folder, the same in every archive seen (17 tracks and trucks):

| Folder | Files |
| --- | --- |
| `ART` | Textures (`.RAW`) and palettes (`.ACT`) |
| `DATA` | The heightmap (`.RAW`), texture map (`.CLR`), texture list (`.TEX`) and the rest of a track's terrain |
| `FOG` | Fog and lighting |
| `LEVELS` | Level files (`.LVL`) |
| `MODELS` | Models (`.BIN`) |
| `TRUCK` | Truck files (`.TRK`) |
| `WORLD` | Situations (`.SIT`, or `.SI2` in tracks converted from Evolution 2) |

The same name can appear in more than one folder, so a lookup must know which folder
it wants and fall back to any other. **Measured:** `MonteCarlo_PZ.pod`, `ROUTE77.POD`,
`rute756jam.pod` and `tightcorners.pod` each hold a 64 x 64 ground texture
`ART\NAME.RAW` beside the 256 x 256 heightmap `DATA\NAME.RAW`, and the texture is listed
first. Taken by name alone, the heightmap comes out as a flat 64-cell world and the texture
map, sized for 256, is refused.

## The base game's archives

A track's or truck's archive can name files that it does not contain: ground textures,
models, palettes, tires and axles. MTM2 finds them in its own archives.

- **Measured** on an English MTM2 CD: 19 archives. 16 are in `Shared`, and 3 are in the
  language folder (`English`): `SOUND.POD`, `TRUCK2.POD` and `UI.POD`. Most of the truck
  parts are in `TRUCK2.POD`, and all 20 of the base game's trucks, one `.TRK` each. The
  palette `METALCR2.ACT` is in `STARTUP.POD`. The base game's 15 tracks are in 13 archives
  in `Shared`: `JUNK.POD` and `SNAKE.POD` hold two each, so a track is named by its archive
  and its track file (`Track::from_file`), not by its archive alone.
- `Shared\POD.INI` lists all 19, after a first line that gives their number (`19`).
  **Measured.** What the game does with the order is **open**.
- **Measured:** 354 file names occur in more than one of the 19 archives, and every copy
  has the same content. So for the base game, the order in which its archives are
  searched does not matter.
- The game looks in the track's or truck's own archive first, in any folder, and then in
  the base archives (`src/base_game.rs`). This is the game's rule. That MTM2 does the same
  is **open**, but no other rule lets a track replace a file of the base game.
- A texture's own palette (see [textures.md](textures.md)) comes only from the archive
  that the texture came from. `AlpineMtns.pod` carries `ROTOR1.RAW` without a
  `ROTOR1.ACT`, and `STARTUP.POD` has a `ROTOR1.ACT`. That palette was measured to go with
  a texture beside it, not with one in another archive.
- What the tracks and trucks examined borrow: `tests/base_game.rs` prints it. MyTrack
  gets all of its 241 ground textures, 4 models and its backdrop from 4 base archives.
  Monte Carlo gets 34 ground textures, 3 models and their 3 textures from 6. `MPNW.pod`
  gets its tires, its axle and 6 textures from `TRUCK2.POD` and `SOUND.POD`. 10 of the 12
  tracks borrow nothing. Critic names one model that no archive holds.

## Open

- Whether any MTM2 archive stores files out of order, overlapping or with gaps. The
  reader does not care, but has not seen one.
- Whether MTM2 lets a track's files replace the base game's, and what `POD.INI`'s order
  does.
