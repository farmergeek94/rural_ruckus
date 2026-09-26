# Trucks (.TRK)

A truck is a truck file, `TRUCK\NAME.TRK`, and the models and textures it names. A
community truck comes as an archive of its own holding those.

**Measured** on the two archives in `trucks/`, and pinned by
`tests/pod_real_trucks.rs`:

| Archive | What it is | Contents |
| --- | --- | --- |
| `99BFoot.pod` | "Bigfoot 1999 Superduty" | 61 files. Self-contained: the truck file, a body, six tire models, an axle, and 26 textures each with its palette. |
| `VirginiaGiant2003.pod` | "Virginia Giant" | 9 files. The truck file, a body, 3 textures and a sound. Its tires (`sil`), axle (`axle3.bin`) and shock texture are the base game's. |

## The truck file

Text, with CRLF line endings, laid out like a track file: a value sits on the line after
the label that names it. The first line is the label `MTM2 truckName`. **Reference:**
Monster Truck Madness 1 files start with a bare `truckName`, which is refused.

`MTM2.1 truckName` starts a truck file for MTM2 Community Patch 3. **Reference**
(JSTruckViewer, `docs/TRK_2_1_FORMAT.md`): the rest of the file is laid out as before,
with two additions. The header is what turns them on: the patched game ignores them in a
file that starts `MTM2 truckName`, even if the files are there.

- A tire model for each wheel. See "Tires" below.
- `superiorAxlebarOffset`, three numbers after the lights: the heights of a second set of
  axle bars above the first. See "Axle bars" below.

**Measured** on `GMC1500TT.POD`: it starts `MTM2.1 truckName`, ends with
`superiorAxlebarOffset` / `200.000000,200.000000,400.000000`, and has the four tire models
that the header asks for.

| Label | Value | Used |
| --- | --- | --- |
| `MTM2 truckName` | The truck's name | yes |
| `truckModelBaseName` | The body's model. Both files write it in full (`bf99_1.bin`). **Reference:** the game's own trucks leave `.bin` off, so it is added when missing. | yes |
| `tireModelBaseName` | What the tire models' names begin with: see below | yes |
| `axleModelName` | The model used for both axles | yes |
| `shockTextureName`, `barTextureName` | Textures for the shock absorbers and the bars that hold the axles | yes |
| `axlebarOffset`, `driveshaftPos` | Three numbers each. Where those bars and the driveshaft meet the body | yes |
| `faxle.rtire.static_bpos.x` and eleven like it | One number each: see below | yes |
| `Scrape point 1 body axis x,y,z` to `12` | Three numbers each: see below | yes |
| `Instrument Cluster` | Name of the dashboard's layout file, without its extension. See [cockpit.md](cockpit.md) | yes |
| `Wave File` | Three lines, each a sound's file name | no |
| `Number of Lights`, then seven labels for each light | Headlights, brake lights and so on. See "Lights" below | yes |

**The file says what a truck looks like and nothing about how it drives**: no mass, no
springs, no engine, no grip. In both files every label is one of those above. Where the
game keeps a truck's handling, if it varies at all, is **open**; here every truck drives
like the built-in one.

### Axes

Feet, from the body model's origin: **X to the truck's right, Y up, Z forwards**, which is
left-handed. **Measured**, in three ways that agree:

- The labels say so: `rtire` has +X and `ltire` -X, `faxle` +Z and `raxle` -Z, and the tires
  are below everything else.
- The body model fits the scrape points, which are given in the same axes, to within a
  foot or so at each end.
- The lettering on Bigfoot's sides only reads from left to right if faces that look
  towards +X are on the truck's right with Z forwards: all 56 such faces agree. In
  right-handed axes with no flip it would read backwards. This is what settles the
  handedness in [conventions.md](conventions.md).

So a truck converts like everything else, (x, y, -z) in metres, and then faces -Z with its
right at +X, as the game's trucks do.

### Tires

`static_bpos` is where the middle of each tire is with the truck standing at rest
("static body position"). Bigfoot: 4.5 ft either side, 3.65 ft below the origin, 6.61 ft
ahead and 6.38 ft behind.

The tire models are named base, detail, side: a base of `BF99_` finds `BF99_16L.BIN` and
`BF99_16R.BIN`, and the same with `10` and `08`. **Reference:** the numbers are how many
sides the wheel has, the coarser ones being drawn at a distance. It can't be seen in this
archive, whose six files are all the same 179-vertex wheel. The finest one present is
used.

An MTM2.1 truck may also have a model for each wheel alone, at the finest detail only:
`BFSW16FL.BIN`, `16FR`, `16RL` and `16RR` for a base of `BFSW`. Where one is present it
takes the place of the side's model on that wheel (**reference**, as above). In
`GMC1500TT.POD` the front two are the same as `16L` and `16R`, and the rear two are
different models. The wheel's size is taken from the model drawn on the front left wheel.

A tire is a wheel centred on its own origin with its axle along X, 3.00 ft in radius
(a 72 inch tire) and 4 ft wide. The left and right models are mirror images. **No field
gives the tire's size**, so the wheel radius is taken from the model.

### The axle

The ends of the axle model's tube are centred on y = 0 of its own origin, to a thousandth
of a foot, while its bounds are not (the differential hangs below and a bracket stands
above). So **its origin goes on the line between the two hubs**, at the middle. Centred
on its bounds instead it would hang 0.93 ft below the wheels it is meant to carry.

### Axle bars

Four bars hold the axles to the body: from each side of the body to the same side of each
axle. The game draws them itself; the truck file gives only where they meet the body,
`axlebarOffset`, in feet in the body's axes, for the right-hand bars. The left-hand ones
mirror them. The GMC's is `1.371250,-2.700000,-0.113281`.

The rest is **reference**, and only as a picture of the game: JSTruckViewer
(`src/worker/truck-worker.js`) rebuilds the bars with numbers of its own, which no file
holds, and the game uses them. In 1/256 ft: a bar meets the body 45 above
`axlebarOffset`, and meets its axle 535 out from the axle's middle, 80 below it and 83
towards the other axle. The bars are drawn 0.1 ft in radius, in `barTextureName`, or grey
where that texture is in the base game's archives. Each bar follows its axle as the
suspension moves.

An `axlebarOffset` 50 ft or more from the body in any direction means "no bars"
(**reference**: the base game's 20 trucks have 2.2 to 3.3 ft, and a car has 999 ft).

An MTM2.1 truck's `superiorAxlebarOffset` adds a second set of four, the first raised by
its three numbers in 1/256 ft: at the front axle, at the rear axle, and at the body
(**reference**, JSTruckViewer `docs/TRK_2_1_FORMAT.md`). The GMC's is `200,200,400`. The
game draws it only for a file that starts `MTM2.1 truckName`.

### Shocks and the driveshaft

Drawn by the game as the bars are, from numbers of its own (**reference**, JSTruckViewer,
as above, in 1/256 ft):

- **Shocks:** two at each wheel, 70 in front of the axle's middle and 70 behind it, 542
  out to the side. Each stands from the body, at the height of its origin, down to 85
  above the axle's middle. 0.12 ft in radius, in `shockTextureName`. Every truck has
  them, whatever its body model already shows: the GMC's body has coil-overs of its own.
- **Driveshaft:** from `(0, y, z)` of `driveshaftPos` on the body to the middle of each
  axle. 0.14 ft in radius, and brown: no file gives it a texture. A `driveshaftPos` of all
  zeros means "no driveshaft" (**reference**: a car writes that).

Like the bars, each follows its axle as the suspension moves, so the drawn suspension
always shows where the wheels are.

### Scrape points

Twelve points on the body in both files: four at each end and four on the roof. By their
name and their places they are where the body touches what it runs into; the game here
makes the truck's collider the convex hull of them. What MTM2 does with them is **open**.

## Textures

**Each texture has a palette of its own**, the `.ACT` of the same name beside it
(`BF99_1.RAW`, `BF99_1.ACT`): all 29 textures in the two archives have one. **Measured**
by looking: through its own palette Bigfoot's paintwork is blue with white lettering, and
through another texture's it is noise. A track, by contrast, has one palette for
everything.

Truck textures are 256 x 256 (65536 bytes) as well as 64 x 64.

A Community Patch 3 truck may have a PNG or TGA in place of a texture, found and used as
a track's is (see textures.md), with no palette. `GMC1500TT.POD` has four PNGs, at
512 x 512 and 256 x 256, and no `.RAW` for them. Its other four textures are `.RAW` with
their palettes.

It also has a normal map beside each PNG: `GMC1500TPY_N.PNG` for `GMC1500TPY`. See
textures.md. The game lays them on every face of a truck that uses the texture.

The GMC's body has 30 faces of type 64 (see model.md): 14 with an alpha-test material,
cut out through the PNG's alpha, and 16 with a blended one, whose base alpha of 0.35 is
the glass of the windows. Those are drawn blended, as see-through as the base alpha
times the PNG's alpha.

## Lights

After `Number of Lights` and its count, each light is six labelled lines, each followed by
its value. The labels name the fields and give their units. **Measured** on the 33 truck
files of the user's archives and of the base game's `TRUCK2.POD` (206 lights): every light
that a file counts reads (`tests/pod_real_trucks.rs`).

| Label | Value |
| --- | --- |
| `Light N type` | An integer. 0, 1, 3, 4 and 5 occur. What each means is **open** (see below). |
| `Light N body axis pos x,y,z (ft), bitmap radius (ft)` | Where the lamp is, in the body's axes, and how big its picture is drawn. |
| `Light N heading (rad), pitch (rad), heading spin speed (rad/sec)` | Which way it shines, and how fast its heading turns. |
| `Light N cone: length (ft), base radius (ft), rim radius (ft), texture name` | The beam: how long, how wide at the lamp and at its end, and its texture. A length of 0 is no beam. |
| `Light N source: bitmap name` | The picture drawn at the lamp. |
| `Light N ms on, ms off (0 if light doesn't blink)` | Two integers. |

- **Heading**: 0 is forwards (+Z), and it turns towards the right (+X): the lamp shines
  along (sin heading, cos heading) across and forwards. **Measured**: of the lamps to one
  side of the middle that are turned at all, 121 are turned outwards under this rule and
  5 inwards; under its mirror image, 5 and 121. Rear lamps have headings near pi, and a
  pair either side of the truck ±pi/2 on the side they are on. Pitch is taken as up from
  level: headlights have -0.17, about 10 degrees down. That the sign is up is **open**.
- **Pictures**: `HEADLITE.RAW` and the brake-light pictures are 64 x 64, with palettes of
  their own beside them: a glow on black. **Measured**. The beam textures `LITEFUZZ.RAW`
  (grey) and `REDFUZZ.RAW` (red) are 256 x 256 noise, in `STARTUP.POD`. MTM2 presumably
  drew the beam as a textured cone. The game draws each lamp's picture as a glow that adds
  to what is behind it, and each beam as a spot light as long and as wide as its cone,
  coloured by the beam texture's mean colour (`src/truck/pod_import.rs`).
- **Types**, by what the lamps of each look like (not proved): 0 are headlights (forward,
  a 75 ft beam, `HEADLITE`), 1 brake lights (backwards, red, no beam), 3 roof lights
  (forward, a 40 ft beam), 4 beacons that turn or blink, and 5 reversing lights
  (`BRLTRV`). Whether MTM2 lights brake lights only when braking and reversing lights only
  when reversing is **open**; the game lights them all when it is dark.
- **Trucks without beams**: 8 of the 12 community trucks examined have no lights, and
  Maximum Destruction's two headlights (type 0) have a beam of length 0 and a picture only
  0.28 ft across. Its rear lamps have pitches of 0.6 and 1.1. **Measured**. The game gives
  every truck a beam of its own (`light_the_road` in `src/truck/pod_import.rs`).

## Open

- A texture with no palette beside it. **Reference:** the game falls back to its own
  `METALCR2.ACT`, which is in its archives (`STARTUP.POD`) and not in a truck's. The game
  does the same, and also when the palette beside a texture cannot be read:
  `08NaturalHigh.pod` and `Slingshot.pod` carry a `BLACK.ACT` of 769 bytes, not 768.
  Whether MTM2 does that for a palette it cannot read is **open**.
- Sounds.
- What each light type means, when each shines, and which way pitch goes.
- Whether pure black is a hole in a truck's cutout faces (type 51) as it is in a track's.
  It is treated the same way.
