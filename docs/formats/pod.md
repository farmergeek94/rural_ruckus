# POD archive

The first-generation Terminal Reality archive, used by MTM2. Little-endian. No magic
number.

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

**Measured** on both archives (20 and 366 entries): the first file starts where the
directory ends, and the files tile the rest exactly, with no gaps and nothing after the
last one.

Files name each other by bare file name in any case (`nutrack.raw` for
`DATA\NUTRACK.RAW`). Lookups ignore case. **Measured.**

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

One name can occur in more than one folder, so a lookup names its folder and falls back
to the others. **Measured:** `MonteCarlo_PZ.pod`, `ROUTE77.POD`, `rute756jam.pod` and
`tightcorners.pod` each hold a 64 x 64 ground texture `ART\NAME.RAW`, listed before the
256 x 256 heightmap `DATA\NAME.RAW`. By name alone, the heightmap reads as a flat 64-cell
world and the texture map, sized for 256, is refused.

No field of an archive says whether it is a track or a truck. Its files tell: it holds a
track if it has a `.SIT` or a `.SI2`, and a truck if it has a `.TRK`. **Measured** on the
29 community archives in `pods/`: 16 hold track files and no truck file, and 13 hold a
truck file and no track file. None holds both, and none holds neither. Nothing seen rules
out one that holds both, so the game lists such an archive with the tracks and with the
trucks. The base game's archives can hold several of one kind (see below).

## The base game's archives

A track's or truck's archive can name files it does not contain: ground textures, models,
palettes, tires and axles. MTM2 finds them in its own archives.

- **Measured** on an English MTM2 CD: 19 archives, 16 in `Shared` and 3 in `English`
  (`SOUND.POD`, `TRUCK2.POD`, `UI.POD`). `TRUCK2.POD` holds most of the truck parts and
  all 20 base trucks, one `.TRK` each. `METALCR2.ACT` is in `STARTUP.POD`. The 15 tracks
  are in 13 archives in `Shared`: `JUNK.POD` and `SNAKE.POD` hold two each, so a track is
  named by its archive and its track file (`Track::from_file`).
- `POD.INI` lists the archives the game loads. **Reference**, MTM2's `README.TXT`
  (sections 1.2 and 1.3): it is in the folder where MTM2 is installed; its first line is
  the number of archives listed; each later line is the path of one archive; a track or
  truck is added to the game by adding a line with its archive's path at the end and
  raising the number. **Measured** on an installed game (`~/Documents/mtm2/POD.INI`, 226
  bytes): `19`, then 19 bare lower-case file names (`startup.pod` ... `summit3.pod`), one
  per line, CRLF line ends, no path. The same 19 names are in `Shared\POD.INI` on the CD,
  3 of which are archives in `English`, so the installer puts both folders' archives in
  one. An archive in the folder that `POD.INI` does not name (the Community Patch's
  `GAME.POD` in that install) is not loaded. What the game does with the order is
  **open**. The reader (`pod::archives_listed`) takes every non-blank line after the
  count as a name, bare or a path, case-insensitive, and skips one that is not there.
- **Measured:** 354 file names occur in more than one of the 19 archives, and every copy
  has the same content. So the search order of the base archives does not matter.
- The game looks in the track's or truck's own archive first, in any folder, then in the
  base archives (`crates/pod/src/base_game.rs`). That MTM2 does the same is **open**, but no other
  rule lets a track replace a file of the base game.
- A texture's own palette (see [textures.md](textures.md)) comes only from the archive
  that holds the texture. `AlpineMtns.pod` carries `ROTOR1.RAW` without a `ROTOR1.ACT`,
  and `STARTUP.POD` has a `ROTOR1.ACT`. That palette was measured to go with a texture
  beside it, not with one in another archive.
- Borrowing, **measured**: MyTrack gets all 241 of its ground textures, 4 models and its
  backdrop from 4 base archives. Monte Carlo gets 34 ground textures, 3 models and their 3
  textures from 6. `MPNW.pod` gets its tires, its axle and 6 textures from `TRUCK2.POD`
  and `SOUND.POD`. 10 of the 12 tracks borrow nothing. Critic names one model that no
  archive holds.

## Open

- Whether any MTM2 archive stores files out of order, overlapping or with gaps. The
  reader does not care, but has not seen one.
- Whether MTM2 lets a track's files replace the base game's, and what `POD.INI`'s order
  does.
