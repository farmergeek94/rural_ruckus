# Texture types (.TTY)

A text file that gives each texture a type number for the kind of surface it shows. The
game uses it for one thing: ice grips less.

**Reference** ([JSPod](https://github.com/juanputrerasm/JSPod), `src/file-type-info.js`):
calls the file "Texture data" and does not read it.

**Reference** ([JSTrackViewer](https://github.com/juanputrerasm/JSTrackViewer) v0.9.9,
commit `81d84e1`, `src/worker/lvl-parser.js`, `parseTty`): splits each number into a
`type` (the hundreds) and a `depth` (the last two digits), uses neither, and names no
MTM2 type. Its surface names (Road, Curb, Grass, Dirt, Rocks, from `CPREDIT.EXE`) are for
CART Precision Racing's `.TTX`, a different file.

Everything else on this page is **measured** on the 12 tracks in `tracks/`.

## Where the file is

The level file does not name it. The name is the heightmap's name with the extension
`.TTY`, in `DATA`: `ALPINE.RAW` has `ALPINE.TTY`, `Junk.raw` has `JUNK.TTY`. All 12
archives have the file there.

## Layout

Text, CRLF line endings.

| Line (from 1) | Content | Example |
| --- | --- | --- |
| 1 | The number of lines that follow | `153` |
| 2 onwards | A texture file name, a comma, and the type number | `11EI4.RAW,901` |

- The count agrees with the lines in all 12 archives. Three converted tracks (Baja Beach,
  Snake River Canyon, Tight Corners) write `0` and no lines.
- A name can be empty: Alpine, Critic and MyTrack each have one line `,0`.
- A name can occur two times with different types. Alpine has three: `CISDA9.RAW` (208
  and 608), `CISDB29.RAW` (208 and 204), `88CRK1.RAW` (318 and 320). The parser takes the
  first. No ground cell of the 12 tracks has one of these textures.
- The list is not the texture list (.TEX). Alpine's list has 153 lines, its .TEX 100
  names, in a different order, and 30 of the 43 textures on Alpine's ground are not in
  its list.

## Type numbers

Values seen: 0; 100, 101; 200 to 205, 208; 301 to 304, 312, 315, 318, 320, 328; 401 to
403, 412; 501; 600 to 606, 608; 702, 703; 800, 801; 901, 902; 1100; 1200 to 1203; 1400.

The hundreds give the kind of surface. The textures, seen through the track's palette or
through their own where they have one:

| Hundreds | Textures looked at | What they show |
| --- | --- | --- |
| 0 | `11EC016` (Alpine) | Rock |
| 3 | `C1WTR`, `88CRK2` (Alpine) | Water, a creek |
| 3 | `ROT11` (Lands Between, 301) | Red ground, not water |
| 6 | `11EC024`, `11EC018`, `11ERRT4`, `GOLFGRAS` (Alpine) | Snow, snow and rock, grass |
| **8** | `11EI7` (Alpine), `SNWICE11` (Lands Between) | **Ice** |
| **9** | `11EI4`, `11EI5`, `FDWATA1` (Alpine) | **Ice** |

Type 1 is on textures named `TRAC*`, `88RD*` and `BSTART*`, road-like names. `TRAC2` and
`88RD1` are in the base game's archives, not Alpine's.

### In the base game

The base game's lists are the best evidence: the game's makers wrote them. The 16 archives
in `Shared` have 15 track files with a type list, of 211 to 669 lines. The textures on the
ground of each, grouped by type (**measured**):

| Hundreds | What the textures on the ground show (base game) | Largest |
| --- | --- | --- |
| 1 | Road: grey asphalt (Junk, Thunder Park), Crazy 98's dark oval | `8BASDS00`, 223 cells |
| 2 | Dirt: dirt with grass, cracked mud, gravel, dirt tracks, soil | `SN4DRC00`, 60,186 cells |
| 3 | Water; 302 is wet stony dirt (Junk, 2 cells) | |
| 4 | Mud: brown, some with stones (Junk) | `JK8DW00`, 21 cells |
| 5 | Sand: pale, orange and red (Outback, Baja) | `8GBSD00`, 57,554 cells |
| 6 | Grass, some with patches of dirt | `CR4GRS00`, 64,880 cells |
| 7 | Rocky ground: orange rock and dirt (Rock Quarry), red sand with bushes (Baja), grey gravel (Junk) | `RQ8TRN00`, 58,879 cells |
| 10 | Metal plates (Summit 2) | 1 cell each |
| 12 | Rock and stone: rocky brown ground, grey rock, stone blocks | `IS8RRK07`, 3,943 cells |
| 14 | Railway track | `TP8TK00`, 256 cells |

Type 0 is on 3 cells only (rock, brick, dirt). No base game track has ice (8 or 9).
Crazy 98's dark oval (`C2RD*`) is type 100: the game's makers called it road. MyTrack has
the same textures. Community tracks do not always agree: Alpine gives its snow 600 and
601, the grass types; Monte Carlo gives Junk's `JK8GR00` 601 where Junk gives 604, and
`JK8DW00` 501 where Junk gives 402. 413,789 ground cells of the 25 track files have a
texture that is not in their list, 65,202 of them in the base game's Snake (`AREA65`).

Every texture on the ground of the 12 tracks, grouped by type (**measured**, with the
base game's archives):

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

So type 101 is asphalt on one track and dirt on another, and asphalt is often not in the
list: Route 77's highway (`COTK023`, `COTK038`, `COTK118`, `COTK119`, grey with a yellow
line) has no type. 347,281 ground cells of the 12 tracks have a texture that is not in
the list.

All five textures with a type from 800 to 999 are ice, and no texture looked at with
another type is ice. The game takes 800 to 999 as ice (`crates/pod/src/track/mod.rs`). Look again
at any new track with a texture of these types. Alpine has 1,408 cells of ice.

## What the game does with the types

Tires throw up dirt and dust only on loose ground (`TrackData::loose_at`).
`crates/pod/src/track/mod.rs` sorts a listed texture by its type: hundreds 2 (dirt), 4 (mud), 5
(sand), 6 (grass) and 7 (rocky ground), from the base game's lists above, are loose
(`Footing::Loose`), so Alpine's snow (600 and 601) is loose; 800 to 999 is ice; every
other type (road, water, metal, rock, railway track, and 0) is firm ground, which throws
nothing up. All grip as well as they can, except ice.

A texture that is not in the list (`Footing::Unnamed`) is loose where its texel under the
tire is coloured and not blue: its red, green and blue differ by 12 or more (from 0 to
255), and blue is not the largest. This is the game's own rule, not MTM2's. Many tracks
need it: Baja Beach, Snake River Canyon and Tight Corners have no list, and most of the
ground of Lands Between (64,871 of 65,536 cells), Route 77 (41,901, its streets and most
of its grass) and tground (30,912, its sand) is not in the list. The share of each
texture's texels that the rule takes as loose (**measured**):

| Texture | What it shows | Loose at 8 | Loose at 12 | Loose at 16 |
| --- | --- | --- | --- | --- |
| `SKTRN028`, `SKTK086` (Baja Beach) | Sand | 100 % | 100 % | 100 % |
| `DVTK128` (Snake River Canyon) | Pale gravel | 100 % | 99 % | 97 % |
| `SAND04` (tground) | Sand | 100 % | 100 % | 100 % |
| `5X5_00` (Tight Corners), `SEG1009` (Route 77) | Grass | 100 % | 97 to 100 % | 94 to 100 % |
| `C6COB00` (Critic) | Mud | 92 % | 91 % | 90 % |
| `GRKDK11` (Lands Between) | Dark grey-green ground | 70 % | 38 % | 13 % |
| `KCOT1` (Route 77) | Warm grey asphalt | 47 % | 10 % | 7 % |
| `8BASD492` (Route 77) | Grey asphalt | 10 % | 1 % | 0 % |
| `4X425` (Tight Corners), `RSTA4` (Critic) | Black asphalt | 4 % | 2 to 4 % | 1 to 3 % |
| `DRSNW11` (Lands Between) | Snow | 0 % | 0 % | 0 % |

Colour cannot fully tell Route 77's warm grey asphalt (red, green and blue about 85, 79
and 74) from Lands Between's grey-green ground (about 76, 74 and 65): at 12, a third of
that ground throws dirt, and a tenth of that asphalt. Yellow lines on road are coloured
and throw dirt. Baja Beach's ground is all loose; the starting grids of Tight Corners and
Route 77, on asphalt, are not.

## Open

- What the other hundreds do in MTM2 (sounds, spray, speed). Only their look is known.
- What the last two digits mean.
- Why Alpine's ice has type 800 in one texture and 901 or 902 in others.
- What a texture that is not in the list gets. Alpine's `11EI6`, an ice edge on 104
  cells, is not in its list. Lands Between lists `SNWICE11`, the middle of a set of nine
  snow and ice textures, and none of the eight edges. The game takes such a texture as
  ground that grips fully and throws dirt up where it is coloured (see above).
- How much MTM2's trucks grip on ice. The game's value (`ICE_GRIP` in `src/footing.rs`)
  is its own.
- Which type the game uses for a texture in the list two times.
