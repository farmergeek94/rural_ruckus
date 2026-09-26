# Ground textures: list (.TEX), images (.RAW) and palette (.ACT)

All **measured** on `AlpineMtns.pod`, which carries its own: a list of 100 textures, of
which its texture map uses 43, all present under `ART\`.

## Texture list (.TEX)

Text, named on line 6 of the level file. The first line is a count, and each line after
it is a texture file name. The texture map's indices count into this list from 0.

## Texture (.RAW)

A square image of palette indices: one byte per pixel, row by row from the top left, with
no header. All 300 in `AlpineMtns.pod` are 4096 bytes, 64 x 64. (The same `.RAW`
extension is used for the heightmap, which is also headerless bytes.)

## Palette (.ACT)

768 bytes with no header: 256 colours of red, green and blue, one byte each, 0 to 255.
Named on line 5 of the level file. The colours are as they appear on screen (sRGB).

The track's palette colours a texture that has no palette of its own (see below).
`AlpineMtns.pod` holds five palettes; the others presumably belong to its models and sky.

### A texture's own palette

**Reference** (JSTrackViewer, `src/worker/palette-resolver.js`): a palette with the same
name as a texture, beside it (`CROKDROP.ACT` beside `CROKDROP.RAW`), colours that
texture, and nothing overrides it. Otherwise the track's palette does.

**Measured** on backdrop textures, the 6 of them in 5 archives that have a palette of their
own: through it, the mean difference between neighbouring pixels is 3 to 27; through the
track's palette, 69 to 177, which is coloured noise (`tests/pod_real_tracks.rs`).

**Measured** on ground and scenery textures too. Lands Between, Route 77, Rute 756 jam,
tground and The Tight Corners give every ground texture a palette of its own. Through it,
the mean difference between neighbouring pixels, averaged over the ground, is 18 to 57.
Through the track's palette, it is 132 to 218. Scenery textures with a palette of their
own are smoother through it on all 9 tracks that have them. A few own palettes are the
same as the track's (Monte Carlo's ground) and make no difference. The game uses the rule
for every texture: ground, ground boxes, scenery and backdrops.

## True-colour textures (.PNG, .TGA), from Community Patch 3

MTM2 Community Patch 3 puts a PNG beside a texture's `.RAW`, or in its place. Files
still name the texture by its `.RAW` name. The game uses the PNG where there is one,
and the `.RAW` where there is not.

**Reference:** JSTrackViewer (`src/worker/palette-resolver.js`,
`src/worker/image-decoder.js`, `src/worker/track-worker.js`), which has its rules from
the Traxx editor's fork:

- The PNG is found by the stem of the `.RAW` name: `SKTK002.RAW` gives `SKTK002.PNG`.
  The game looks in `ART`, `MODELS`, `DATA` and `TEXTURES` in that order, then in any
  folder.
- The file's own first bytes, not its extension, say that it is a PNG.
- The patched game takes only square images whose side is a power of two, from 32 to
  1024 pixels. The game refuses any other image, and uses the `.RAW`, if there is one.
- A PNG needs no palette. Its rows run from the top, like a `.RAW`'s: it takes the
  `.RAW`'s place without a flip.
- On a cutout face, a PNG's own alpha makes the holes, cut at a half. The game does not
  make pure black into a hole, as it does for a `.RAW`. On every other face the PNG is
  opaque. The ground is always opaque.
- A TGA is looked for where there is no PNG, in the same folders. It has no signature;
  the game takes one whose header says true colour (type 2, or 10 run-length encoded),
  24 or 32 bits, and whose name ends `.TGA`. Its rows run from the bottom unless bit 5 of
  the header's last byte says otherwise (Truevision TGA 2.0). No archive here has one:
  the reading is tested with made-up files only.
- **Normal maps:** `<stem>_N.PNG`, or `<stem>_N.TGA`, beside a texture (**reference**,
  JSTruckViewer `docs/BIN_HD_FORMAT.md`). No file names them. Red, green and blue are a
  direction across the texture, down it (DirectX's way, which is the opposite of
  Bevy's, so the game flips green) and out of the surface, each 0 to 255 for -1 to 1.
  Alpha is never read. The game uses them on trucks only; `GMC1500TT.POD` has four.

**Measured** on `BAJBEACH_MTM2_HD.POD` and `TDSNAKE.POD`, which have no `.RAW`
textures at all: every ground texture in their lists has a PNG, all 64 x 64 RGBA. Their
model textures are 256 x 256, with one 128 x 128 in each. Baja's palm leaves are
transparent over 31% of their texture, and its huts and ground are opaque throughout.

Ground tiles must all be the same size, as before. Scenery tiles are brought to the size
that most of a track's model textures have: they are made smaller by averaging, as for
mipmaps, and made larger by repeating each pixel.

## Laying a texture on a cell

The texture map (see terrain.md) gives each cell a texture, two mirror bits and a number
of quarter turns. With `u` running 0 to 1 across the cell along +X and `v` along +Z:

1. Unturned, the texture's **top edge lies along the cell's far (+Z) side** and its left
   edge along the near (x = 0) side. So (u, v) is at (u, 1 - v) in the image, measured
   across and down from its top left.
2. **Mirroring comes first.** Bit 12 flips the texture top to bottom. Bit 13 flips it
   left to right.
3. **Then the quarter turns**, bits 14 to 15, each taking the point (s, t), measured from
   the texture's centre with t upwards, to (-t, s).
4. Tiles simply sit side by side. There is no overlap between neighbours to crop.

`TextureCell::texture_coords` implements this.

### How this was measured

Road and shoreline tiles are drawn to continue into their neighbours, so the right rules
are the ones that make the colours either side of each cell edge agree best. All 64
combinations of starting orientation (8), turning direction (2), meaning of the mirror
bits (2) and order of mirroring and turning (2) were scored on the 14784 cell edges of
`AlpineMtns.pod` that touch something other than open ground.

| Rules | Mean colour jump across edges (0 to 255) |
| --- | --- |
| As above | 31 |
| Best with any other starting orientation | 46 |
| Turning the other way | 71 |
| Texture upside down | 55 |
| Mirror bits swapped (scored on edges touching mirrored cells) | 59, against 31 |
| Turning before mirroring (same edges) | 42, against 31 |
| Neighbouring tiles overlapping by 1 to 4 pixels | 35 to 43, against 31 |

For scale, neighbouring rows of pixels inside a single tile differ by 18, and many of the
scored edges are between tiles that were never meant to match, such as road against
grass. `tests/pod_real_tracks.rs` holds the winner against its rivals, and
`tests/pod_track.rs` can paint a map of the converted ground to look at, on which roads
and their curved edges run unbroken.

Only 117 of the 65536 cells are mirrored, 62 of them also turned, so the order of
mirroring and turning rests on the least evidence.

## Open

- Whether the whole picture is mirrored. Edge matching cannot tell: a mirror image of
  the world lines up just as well. It depends on the Z flip in conventions.md.
- Textures of other sizes, and tracks that use the base game's textures.
- Tracks whose PNGs are a different size from their `.RAW` textures. Traxx samples
  such a ground tile as if it were the size of the `.RAW` (reference); the game
  samples the whole tile, so this does not matter here yet.
- The lighting table (.LTE), and the `.CL1`, `.CL2` and `.RA2` to `.RA5` files. `.RA0`,
  `.RA1` and `.CL0` are not detail levels, as was once thought, but the ground boxes:
  see [ground_boxes.md](ground_boxes.md).
- How the original game filtered and lit the ground. The game here filters smoothly,
  with mipmaps and anisotropic filtering, which the original's software renderer did not.
