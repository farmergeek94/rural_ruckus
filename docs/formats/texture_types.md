# Texture types (.TTY)

A text file that gives each texture a type number. The type tells the kind of surface that
the texture shows. The game uses it for one thing: ice grips less.

**Reference** ([JSPod](https://github.com/juanputrerasm/JSPod), `src/file-type-info.js`):
JSPod calls the file "Texture data" and does not read it.

**Reference** ([JSTrackViewer](https://github.com/juanputrerasm/JSTrackViewer) v0.9.9,
commit `81d84e1`, `src/worker/lvl-parser.js`, `parseTty`): reads the file, and splits each
number into a `type` (the hundreds) and a `depth` (the last two digits). It uses neither,
and it names no MTM2 type. Its surface names (Road, Curb, Grass, Dirt, Rocks, from
`CPREDIT.EXE`) are for CART Precision Racing's `.TTX`, which is a different file.

Everything else on this page is **measured** on the 12 tracks in `tracks/`.

## Where the file is

The level file does not name it. The name is the heightmap's name with the extension
`.TTY`, in `DATA`: `ALPINE.RAW` has `ALPINE.TTY`, `Junk.raw` has `JUNK.TTY`. This is
true in all 12 archives. All 12 have the file.

## Layout

Text, CRLF line endings.

| Line (from 1) | Content | Example |
| --- | --- | --- |
| 1 | The number of lines that follow | `153` |
| 2 onwards | A texture file name, a comma, and the type number | `11EI4.RAW,901` |

- The count agrees with the number of lines in all 12 archives. Three converted tracks
  (Baja Beach, Snake River Canyon, Tight Corners) write `0` and no lines.
- A name can be empty: Alpine, Critic and MyTrack each have one line `,0`.
- A name can be in the list two times with different types. Alpine has three:
  `CISDA9.RAW` (208 and 608), `CISDB29.RAW` (208 and 204), `88CRK1.RAW` (318 and 320).
  The parser takes the first. No ground cell of any of the 12 tracks has one of these
  textures, so the choice has no effect on them.
- The list is not the texture list (.TEX). It has a different order and different
  textures. Alpine's list has 153 lines and its .TEX has 100 names. 30 of the 43
  textures on Alpine's ground are not in its list at all.

## Type numbers

Values seen: 0; 100, 101; 200 to 205, 208; 301 to 304, 312, 315, 318, 320, 328; 401 to
403, 412; 501; 600 to 606, 608; 702, 703; 800, 801; 901, 902; 1100; 1200 to 1203; 1400.

The hundreds seem to give the kind of surface. We compared the numbers with the textures,
seen through the track's palette or through the texture's own palette where it has one:

| Hundreds | Textures looked at | What they show |
| --- | --- | --- |
| 0 | `11EC016` (Alpine) | Rock |
| 3 | `C1WTR`, `88CRK2` (Alpine) | Water, a creek |
| 3 | `ROT11` (Lands Between, 301) | Red ground, not water |
| 6 | `11EC024`, `11EC018`, `11ERRT4`, `GOLFGRAS` (Alpine) | Snow, snow and rock, grass |
| **8** | `11EI7` (Alpine), `SNWICE11` (Lands Between) | **Ice** |
| **9** | `11EI4`, `11EI5`, `FDWATA1` (Alpine) | **Ice** |

Type 1 is on textures named `TRAC*`, `88RD*` and `BSTART*`, which look like road names.
`TRAC2` and `88RD1` are not in Alpine's archive: they are in the base game's archives.

We also looked at every texture that is on the ground of the 12 tracks, grouped by type
(**measured**, with the base game's archives):

| Type | Textures on the ground | What they show |
| --- | --- | --- |
| 100 | 87 `C2RD*` (MyTrack), 310 cells | A dark brown oval with a white edge line, and grass |
| 101 | `ZSTRD3`, `ÑCALLE1`, `PGALF549`, `ZRDTEX1` (rute756jam) | Grey asphalt, with lane lines on three |
| 101 | `C2RD55`, `C2RD56` (MyTrack) | The same dark brown oval as type 100 |
| 101 | `C8OF208` (MyTrack), 1 cell | Dirt |
| 201 | `CODR*` (Route 77), `LYNDRT11` (Lands Between) | Grass and bare dirt |
| 203 | `C8OFF*`, `C8DRD*` (MyTrack) | Dirt, and dirt and grass |
| 205 | `GY8CR81`, `REO*` (Monte Carlo) | Dirt, and dirt and rock |
| 501 | `LG7TAN01` (rute756jam), `JK8*` (Monte Carlo) | Sand, brown ground, rock |
| 1201 | 14 textures | Grey cracked ground, and grass or dirt |

So type 1 is not only asphalt: type 101 is asphalt on one track and dirt on another. And
asphalt is often not in the list: Route 77's highway (`COTK023`, `COTK038`, `COTK118`,
`COTK119`, grey with a yellow line) has no type. Of the ground cells of the 12 tracks,
347,281 have a texture that is not in the list.

All five textures with a type from 800 to 999 are ice. No texture that we looked at with
another type is ice. `tests/pod_real_tracks.rs` checks this: it fails if a new track has a texture of
these types that nobody looked at. The game takes 800 to 999 as ice
(`track/pod_import.rs`). Alpine has 1,408 cells of ice.

## Open

- What the other hundreds do in MTM2 (sounds, spray, speed). Only their look is known.
- Whether MyTrack's dark oval (types 100 and 101) is asphalt or packed dirt.
- What the last two digits mean.
- Why Alpine's ice has type 800 in one texture and 901 or 902 in others.
- What a texture that is not in the list gets. Alpine's `11EI6`, an ice edge on 104
  cells, is not in its list. Lands Between lists `SNWICE11`, the middle of a set of nine
  snow and ice textures, and none of the eight edges. The game takes a texture that is
  not in the list as firm ground.
- How much MTM2's trucks grip on ice. The game's value (`ICE_GRIP` in `src/footing.rs`)
  is its own.
- Which type the game uses for a texture in the list two times.
