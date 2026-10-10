# Cockpit layout (`POWERBIG.480`) and its pictures

The dashboard of the in-cab view. A truck file names it on the line after
`Instrument Cluster` (see [truck.md](truck.md)). The layout tells where the pictures,
dials, steering wheel and mirror go on the screen.

No reference project reads these files, and nothing here comes from the game's program.
Every fact is **measured** on the base game's `COCKPIT.POD` (in `Shared`) and on the 33
truck files in `trucks/` and the base game's archives, unless marked **open** or **Own**
(the game's own choice). The parser is `crates/pod/src/cockpit.rs`.

## Where the files are

- All 33 truck files name `powerbig`; no other name occurs.
- Only `COCKPIT.POD` holds layouts: `DATA\POWERBIG.480`, `.400` and `.200`, one per
  screen size. The pictures and palettes are in `ART\`, the needle model in `MODELS\`.
- **Own**: the game reads `<name>.480`, and `.400` then `.200` if it is missing or has no
  background. Which one MTM2 picked for a screen size is **open**.

| Extension | Screen (pixels) | Why |
| --- | --- | --- |
| `.480` | 640 x 480 | `PBIG480.RAW` is 307,200 bytes. The 3D window is `0,88,640,240`. |
| `.400` | 320 x 400 | `PBIG400.RAW` is 128,000 bytes. The 3D window is `0,72,320,200`. |
| `.200` | 320 x 200 | `PBIG200.RAW` is 64,000 bytes. The 3D window is `0,36,320,100`. |

## Layout

Text, CRLF line endings, a blank line at the end. Each field is a comment line, `;` and
the field's name, then its value lines. The names are the same in all three files; some
carry notes, such as `; Speedometer zero angle (was 315.0)`. The parser finds a field by
the start of its name, without case. A missing or unreadable field gives no value, and
the others are still read.

Numbers are screen pixels from the top left, y down. A rectangle is left, top, width and
height unless the table says otherwise.

| Field (after `;`) | `.480` value | Meaning |
| --- | --- | --- |
| `Background image file` | four lines: `pbig480.raw`, `pbigl480.raw`, `pbigr480.raw`, `pbigb480.raw` | The pictures looking ahead, left, right and back. See "Backgrounds". |
| `3D Window coordinate and size` | `0,88,640,240` | Where the road shows. A rectangle. |
| `Speedometer center` | `153,319` | Where the needle turns. |
| `Speedometer radius` | `32` | How long the needle is. |
| `Speedometer needle model` | `needle.bin` | A model (see [model.md](model.md)). The game does not use it. |
| `Speedometer zero angle (was 315.0)` | `304.5` | See "Dials". |
| `Speedometer degrees per mph : 238.5 / 90 = 2.65 (was 2.6)` | `2.65` | See "Dials". |
| `Speedometer face redraw coordinate and size` | `124,296,19,47` | A rectangle on the left of the dial. What it is for is **open**. |
| `Tachometer center`, `radius`, `needle model` | `435,307`, `24`, `needle.bin` | As for the speedometer. |
| `Tachometer zero angle (was 157.5)` | `153.0` | See "Dials". |
| `Tachometer degrees per rpm : 235 / 9000 = 0.0261 (was 0.0257)` | `0.0261` | See "Dials". |
| `Tachometer face redraw coordinate and size` | `438,287,12,45` | **Open**, as for the speedometer. |
| `Steering wheel coordinate and size` | `145,330,350,150` | A rectangle. The wheel's pictures are this size. |
| `Erase window` | `174,362,466,480` | Four numbers. They look like two corners, not a size. **Open**. |
| `Steering wheel base filename` | `PW480` | See "Steering wheel". |
| `Number of mirrors` | `1` | Then the fields of each mirror, in turn. |
| `Mirror Location, size` | `535,120,105,48` | Where the mirror's view goes. A rectangle. |
| `Angles` | `0,0,32768` | **Open**. 32768 is half of 65536, which is half a turn if a turn is 65536: a mirror looks back. Which number is which turn is not known. |
| `Translation` | `0,0,0` | **Open**. |
| `Zoom` | `49152` | **Open**. It is 3/4 of 65536. |
| `Bitmap position and size` | `532,115,108,56` | Where the mirror's frame picture goes. A rectangle. |
| `Bitmap name` | `fordm480.raw` | The mirror's frame picture. |
| `Shifter name` | `ps480` | The gear shifter's pictures: `PS4801`, `2`, `3`, `N`, `P`, `R`. |
| `Shifter position and size` | `496,272,128,192` | A rectangle. |
| `Shift light bitmap` | `pl480.raw` | 18 x 19, with `PL480.ACT`. |
| `Shift light position and size` | `378,261,18,19` | A rectangle. |

The `.400` and `.200` files have the same fields, with their own numbers and names.

## Pictures

A cockpit picture is a `.RAW` like a texture (see [textures.md](textures.md)): one
palette index per pixel, rows from the top, no header, not square. The layout gives its
size, which agrees with the bytes.

- **Palettes.** Each background and steering wheel picture has its own `.ACT` of the same
  name beside it; all 25 palettes of the `480` pictures are different. `FORDM480.RAW`
  (the mirror frame) and the six `PS480*.RAW` shifter pictures have none. Which palette
  colours them is **open**: through `PBIG480.ACT` the shifter pictures are noise.
- **See-through: palette index 0.** In each of the four backgrounds, index 0 is the only
  black in the palette, and every index 0 pixel is inside the 3D window: 111,271 of the
  window's 153,600 pixels in `PBIG480.RAW`, the rest being the roll cage and the dials.
  In each steering wheel picture the top corners are index 0 and the hub is not.
  Rendered, index 0 is the windscreen and the space round the wheel. The game draws it
  as clear (`pod::SEE_THROUGH_INDEX`, `crates/pod/src/truck.rs`).
- **`.AAI` files.** Most pictures have one beside them (`PBIG480.AAI`, `PW480C00.AAI`):
  a 32-bit number 1, a 32-bit count, then that many records of four 16-bit numbers. The
  length is 8 + 8 x count in all 61 files. A record's first two numbers are x and y on
  the picture; the second two are x and y too, and the top bit of the last is sometimes
  set. Every point is next to an index 0 pixel: the records follow the edge of the
  see-through area, perhaps to smooth it. Their purpose is **open**. The game does not
  read them.

## Backgrounds

The four backgrounds are ahead, left, right and back, in the layout's order. Their names
say so (`PBIG`, `PBIGL`, `PBIGR`, `PBIGB`), and the pictures agree: the right view has the
ahead view's dials on its left, and the left view has the red stripe of the ahead view's
left pillar on its right. Only the ahead view has the dials and the steering wheel.

## Dials

A needle points at `zero angle + degrees per unit x value`, in degrees **clockwise from
straight up** on the screen. Units are miles per hour for the speedometer and revolutions
a minute for the tachometer.

Measured on the dial of `PBIG480.RAW`, unwrapped round `Speedometer center`: the long
white marks just past the needle's tip (radius 35 to 40) are at 330.5, 358.5, 24.2, 50.8,
76.5, 101.5, 127.5, 154.8, 181.5 and 209.8 degrees clockwise from up. The rule gives
331.0, 357.5, 24.0, 50.5, 77.0, 103.5, 130.0, 156.5, 183.0 and 209.5 for 10 to 100 mph.
The printed numbers agree: 20 is straight up and 90 straight down. On the tachometer,
1,000 rpm is straight down (179.1 degrees) and 8,000 rpm straight up (1.8 degrees), where
the numbers 1 and 8 are. `the_dials_turn_clockwise_from_straight_up` checks that each
mark has a bright mark within 3 degrees; counted the other way, most have none. The
file's notes agree: 238.5 degrees for the 90 mph from 10 to 100, and 235 degrees for
9,000 rpm.

## Steering wheel

The pictures are the base name and `C00`, or `L` or `R` and a number: `PW480C00`,
`PW480L05` to `PW480L35` and `PW480R05` to `PW480R35`, in steps of 5. The game looks for
numbers up to 90. Each is 350 x 150 with its own palette. An `L` picture is turned to the
left (anticlockwise). Whether the number is in degrees is **open**: the spokes of `L35`
look turned by more than 35 degrees.

## What the game does with it

`crates/pod/src/truck.rs` makes `truck::Dashboard` from it; `camera/dashboard.rs` draws it.

- The picture is stretched over the whole window, whatever its shape (**Own**; MTM2 had
  a 4:3 screen): across by the window's width over 640, down by its height over 480.
- Index 0 is clear and the 3D view fills the whole window, not the 3D window alone as in
  MTM2. The eye is tipped down a little, so that a level horizon is 40% of the way down
  the 3D window (**Own**).
- The dashboard is drawn over the scene, unlit: as bright at night as by day (**Own**).
- Looking ahead: the steering wheel picture nearest to the steering, full steering being
  the largest number (**Own**), and a needle on each dial. The needles are thin bars in
  the game's blue, not `NEEDLE.BIN` (**Own**), and go no more than 300 degrees round. On
  a stretched screen a needle points at the same mark on the stretched dial, and its
  length changes to reach it.
- The truck has no engine speed. The tachometer shows 1,000 rpm at rest and up to 7,000
  rpm at top speed (**Own**).
- Not drawn: the mirror, the shifter, the shift light. The mirror needs a second camera,
  which the other slices' single-camera queries do not expect. The shifter's palette, and
  when the shift light is lit, are **open**.

## Open

- Which layout MTM2 used for which screen size.
- The face redraw rectangles, the erase window, and the `.AAI` files.
- The mirror's angles, translation and zoom. The palette of the mirror frame and the
  shifter pictures.
- The unit of the steering wheel pictures' numbers.
- How MTM2 placed the 3D view in its window (its field of view, and where its horizon
  was).
