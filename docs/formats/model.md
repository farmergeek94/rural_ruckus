# Models (.BIN)

Little-endian throughout. A model is a stream of records, each a run of 32-bit integers
that begins with a number saying what it is, up to an end record. There is no header
as such: the first record is always a magnification and the second a vertex list.

**Measured** on the 32 models of `AlpineMtns.pod`, all of which read through to their end
record, 3464 faces in all, and on the 9 truck models of `99BFoot.pod` and
`VirginiaGiant2003.pod`, 4119 faces. The record lengths were first taken from **reference**
(JSTrackViewer, which has them from the game engine's own table by way of the Traxx
editor's source) and every length that occurs in these files is confirmed by that.

## Records that occur

| Type | Name | Layout after the type | Notes |
| --- | --- | --- | --- |
| 20 | Magnify | 1 int | Always 65536 so far. Vertex units scale inversely with it (**reference**). |
| 2 | Vertex list | slot, count, then count x (x, up, z) | The first list is the model. |
| 3 | Normal list | slot, count, then count x (x, up, z) | Trucks only, straight after the vertex list: see below. |
| 13 | Texture | slot, then a 16-byte name | In force for the faces that follow. |
| 29 | Texture cycle | slot, count, frame, rate, time, flag, then count x 32-byte names | An animated texture: see below. |
| 10 | Colour | 1 int | For the untextured faces that follow. Only black occurs, so its byte order (taken to be a Windows COLORREF) is **open**. |
| 24, 17 | Textured face | corner count, 3-int normal, 1 int, then count x (vertex, u, v) | 3088 and 368 of them in Alpine. Type 17 is a cutout: see below. |
| 41, 51 | Textured face, shaded smoothly | the same | Trucks only: 3646 and 30 of them, beside 443 of type 24. Type 51 is a cutout. That these two are the smooth ones is **reference**: the engine's names for 24 and 41 differ by a G, for Gouraud. |
| 25 | Untextured face | corner count, 3-int normal, 1 int, then count x vertex | 8 of them. |
| 63 | Material | flags, then 10 ints | Community Patch 3. In force for the faces of type 64 that follow. See below. |
| 64 | Textured face, with a material | the same as type 24 | Community Patch 3. Takes the material in force. |
| 12 | Order | 6 ints | Stepped over. Once, in the helicopter. See below. |
| 18 | Jump | 1 int | Stepped over. Three times, in the helicopter. See below. |
| 0 | End | | |

Types 62 (a texture with a 64-byte name, read like type 13), 65 (4376 bytes) and 66
(32 bytes) are also from Community Patch 3. Their lengths are **reference** only
(JSTruckViewer, `docs/BIN_HD_FORMAT.md`): none has been seen. Type 67 is unassigned: it
was given to a normal-map record and then taken back, so it stops the read like any
unknown type. Other types in
the engine's table (**reference** only, none seen) are skipped by their
known length, or stop the read with what was read so far kept and `Model::incomplete`
saying why. A file that starts with 0x20 is not a model but an animation control file
naming the models that are its frames: see below.

## Vertices

Three ints each: **x, up, z, in 1/256 ft**, relative to the model's origin, in MTM2's
left-handed axes. **Measured**, and tied to the terrain measurements: from the heightmap
and track file alone, each model was found to sit a constant height above the ground
(terrain.md). That height is exactly how far the model reaches below its origin at 256
units to the foot, because the editor stands a model on its lowest point:

| Model | Sits above ground | Lowest vertex / 256 |
| --- | --- | --- |
| `GRDRAIL2` | 3.0 ft | -3.00 |
| `13ETREE1` | 11.0 ft | -11.00 |
| `13ETREE2` | 6.0 ft | -6.00 |
| `CASSIGA` | 10.0 ft | -10.00 |
| `88WRNSN1` | 10.9 ft | -10.91 |
| `DOZER2` | 8.0 ft | -8.00 |
| `TIRRV21` | 2.5 ft | -2.50 |
| `18ESNCAT` | 6.0 ft | -6.00 |

(Two others, with two placements each, were set by hand and differ by about a foot.)

## Vertex normals

Record 3 is in most truck models and in no track model. It follows the vertex list and
holds **one normal for each vertex**, in the same axes, as three ints of length 65535
like the normals stored in faces. **Measured**: the count equals the vertex count in all
9 of Bigfoot's models, and the next record always lands on a known type. Max-D's
`DRVSHAFT.BIN` has no normal list at all: its vertex list is followed by the texture and
by flat-shaded faces (type 24), which need none. On Bigfoot's tire all 179 are
of unit length, 155 point away from the hub (the rest are the dished wheel), and at all
688 corners the normal is on the side that the face's winding says is out. **Reference**
agrees as far as it goes: JSTrackViewer calls the record `MRGL_ILIST` and steps over it at
12 bytes a vertex, and calls face type 41 `ZGFACETTMAP`, the G being Gouraud shading.

A normal can be on the wrong side of a face all the same. Of the 2476 faces of Bigfoot's
body, 90 are, and 80 of those are the back of a panel that is meant to be seen from both
sides: the same corners listed twice, once each way round, sharing vertices that can only
have one normal. (A further 8% of the body's normals are zero.) So the winding is what
says which side a face is seen from, and a vertex normal is used only where it agrees
with its face, the face's own normal otherwise.

## Faces

- Corners are listed in order round the face, 3 or 4 of them in these files.
- **Winding: measured.** Taken with the right-hand rule on the numbers as written, the
  corner order gives the normal stored in the face, for 3332 of 3464 faces. The other
  132 are four-cornered faces that aren't flat. Because the game flips Z, which turns
  every face inside out, it takes the corners in the opposite order.
- The stored normal has a length of 65535. The int after it is not understood and not
  needed.
- **Texture coordinates** are 16.16 fixed point, with 0 to 255 (`0xff0000`) spanning the
  texture whatever its size in pixels, as (across, down) from its top left. **Measured**
  in that 0 and `0xff0000` are by far the commonest values. `METLCRN1` has 30 corners
  holding what is plainly leftover text from its authoring tool, so out-of-range values
  must be tolerated; the game clamps them.
- **Cutouts: reference.** MTM2 has no alpha channel. On faces of type 17 (and 51), texels
  that come out pure black through the palette are holes. On other faces black is black.
  A face of type 64 is a cutout if its material says so, and a PNG texture uses its alpha
  for the holes (see textures.md).
- Textures are 64 x 64, 8-bit, through the track's palette, like the ground's. A truck's
  are up to 256 x 256, each through a palette of its own: see [truck.md](truck.md).

## Materials (Community Patch 3)

A material record is 12 ints with its type, **reference** (JSTrackViewer,
`src/worker/bin-decoder.js`). **Measured**: at that length every model in
`BAJBEACH_MTM2_HD.POD`, `TDSNAKE.POD` and `GMC1500TT.POD` reads exactly to its end
record, 62 models in all. Only a face of type 64 takes the material. Faces of other types
that follow a material record ignore it (**reference**). Snake River and the GMC truck have
such faces.

The first int is flags, and the fifth is the base alpha, in 16.16 fixed point
(**reference**, JSTruckViewer `docs/BIN_HD_FORMAT.md`). The game uses three flags:

| Flag | Meaning | Game |
| --- | --- | --- |
| `0x0004` | Blend: see-through by the base alpha and the texture's alpha | A blended face, on trucks. Scenery draws it opaque |
| `0x0008` | Alpha test: see-through where the texture's alpha is under a half. Takes precedence over blend | A cutout face |
| `0x0080` | Two-sided | Drawn from both sides |

The flags that occur are `0x83` and `0x8f` (Baja, whose `0x8f` is its vegetation),
`0x81` and `0x1089` (Snake River), and `0x16b` and `0x167` (the GMC truck, whose base
alpha of 0.35 is glass). The other bits (lit, additive, no depth write, emissive, tint),
and the other ints, are not used yet.

A two-sided face can also have its back written in the file. **Measured** on Baja Beach:
in 8 of its models (the four trees, the two huts, the ship and the plane), 554 faces have a second face with
the same corners in the reverse order. Both faces are two-sided, and the texture positions
are different. The importer draws the back of a two-sided face only when the model does not
have that face in the reverse order. If it did, the two backs are in the same position
with different textures, and they flicker. `tests/pod_track.rs` examines this on all
tracks. No other track or truck has such faces.

Some files also write a face two times, in the same order. On Baja Beach the copies
are the same (the fence and the raft). On Rute 756 Jam (`BFVPROP.BIN`) they have
different textures. How the game selects between those is **open**. The importer
draws them as the file gives them.

## Animated textures (record 29)

A texture record that names more than one texture. The layout and length (32 x count +
28 bytes) are **reference** (JSTrackViewer, `src/worker/bin-decoder.js`, which calls it
MRGL_TEXTURECYCLE and takes its first name), and **measured**: every model that has one
reads to its end record at that length, and the names are texture files in the archive.

| Offset | Field | Notes |
| --- | --- | --- |
| 0 | 29 | |
| 4 | ? | 0 in every file seen. **Open**. |
| 8 | count | How many names: 2 to 8 in our files. |
| 12 | ? | 0 in every file seen. **Open**. |
| 16 | rate | A number that differs from cycle to cycle. See below. |
| 20, 24 | ? | 0 in every file seen. **Open**. |
| 28 | names | count x 32 bytes, each a file name that ends at its first NUL. Some files have rubbish after the NUL. |

| Model | Where | Rate | Names |
| --- | --- | --- | --- |
| `HELI.BIN` | Alpine Mountains | 1024 | `rotor1.raw` to `rotor8.raw` |
| `88WRNSN1.BIN` | Alpine Mountains | 43690 | `88light1.raw`, `88light2.raw` |
| `8CONSIGN.BIN` | Critic | 32767 | Two records: `8DLITE`, `8BLITE`, and the same two the other way round |
| `SUPRAAN1.BIN` | Rute 756 Jam | 4369 | `LAGPACE2` to `LAGPACE4` |
| `TXSMKA.BIN` | Rute 756 Jam | 6500 | `TXSMK0` to `TXSMK7` |
| `TXSTMA.BIN` | Rute 756 Jam | 13000 | `TXSTM0` to `TXSTM7` |
| `CAPUSA-1.BIN`, `CARO4-1.BIN`, `STBLZR.BIN`, `NRS_1.BIN` | Truck bodies of Captain USA, CARO4-1, Stabilizer, 2011 Rock Star | 2729 | `AENGANI1.RAW`, `AENGANI2.RAW`, 256 x 256 |

The names read as the frames of an animation (`rotor1` to `rotor8`, `88light1` and
`88light2`). How MTM2 steps through them, and what the rate means, is **open**.

**Own.** The game here shows the frames in the order written, round and round, each for
the same time, with no blending, all on one clock. It reads the rate as 16.16 fixed point
and uses that as seconds per frame: 1024 is 1/64 s for the rotor, 43690 is 2/3 s for the
warning light. That reading is the game's own choice. What supports it is only that
several rates are simple fractions of 65536 (1024, 32767 and 43690 are 1/64, about 1/2
and about 2/3 of it), as the 16.16 numbers of this format are (texture coordinates, above, and a
material's base alpha); no file shows the unit. The scenery puts a cycle's frames in
consecutive tiles and steps through them in its material (`track/tiles.wgsl`); a truck
switches the texture of the material (`truck/looks.rs`). The backdrop shows the first
frame. `tests/pod_real_tracks.rs` and `tests/pod_real_trucks.rs` check that every frame is
in its archive.

## Order and Jump (records 12 and 18)

Both are stepped over by their lengths, 7 and 2 ints with the type (**reference**,
JSTrackViewer, `src/worker/bin-decoder.js`, which also notes that the game follows a jump
when it draws, rather than stepping over it). What the Order's numbers mean is **open**.

The only model with them is Alpine's `HELI.BIN`. **Measured**: its Jump's second int,
added to where the Jump starts, lands on the start of a record, for all three: the first
jumps from after the vertices to the Order near the end, the other two to the end record.
The Order's last two ints, added likewise, land on the starts of the body's first record
and of the rotor's animated texture. Read straight through, stepping over both, the
helicopter reads every face once and to its end record, and the rotor's faces take the
animated texture: `tests/pod_real_tracks.rs` checks this. What a model would need if it
had faces that a Jump jumps over is **open**; no such model has been seen.

## Animation control files (keyframes)

A .BIN whose first int is 32 (0x20) is not a model but a list of the models that are the
frames of one. **Reference** (JSTrackViewer, `src/worker/bin-decoder.js` and
`src/worker/track-worker.js`): the count is at int 2, the names are 16 bytes each from int
6, the record is 344 bytes (86 ints), and the frames are separate models. **Measured** on
the four in our files:

| Offset | Field | Notes |
| --- | --- | --- |
| 0 | 32 | |
| 4 | ? | 0 in every file. **Open**. |
| 8 | count | 4 or 8. |
| 12 | rate | 65536, 32768, 16384 or 81920. See below. |
| 16, 20 | ? | 0 in every file. **Open**. |
| 24 | names | 16 bytes each, NUL-padded file names of the frame models, with `.bin`. Zero after the last. |
| 344 | 0 | An end record. Every file is 348 bytes. |

Every frame is a model in the same archive, and all the frames of one file have as many
vertices as its first (**measured**, `tests/base_game.rs` and `tests/pod_real_tracks.rs`).
They also have as many records of each type, which was read from the files but no test
pins. So the frames of one animation read as one model with
its vertices in other places.

| File | Archive | Frames | Vertices | Rate | Placed |
| --- | --- | --- | --- | --- | --- |
| `REX.BIN` | `CRAZY98.POD` | `rex1` to `rex4` | 822 | 65536 | Crazy '98 |
| `PUMPJACK.BIN` | `OUTBACK.POD` | `pj0` to `pj7` | 286 | 32768 | 5 times in `AUSSIE.SIT` |
| `OP88ANIM.BIN` | `Critic.pod` | `op88_1` to `op88_8` | 48 | 81920 | Once in `000CRIT.SIT` |
| `TERYL.BIN` | `STARTUP.POD` | `teryl1` to `teryl4` | 204 | 16384 | Not placed by any track seen |

A track places a control file exactly as it places a model: the box's model line names it
(**measured**). How MTM2 moves between the frames, and what the rate means, is **open**.

**Own.** The game draws the first frame's faces, textures and texture positions, and
moves each vertex in a straight line from where one frame puts it to where the next does,
round and round from the last to the first, on one clock, so that every copy of a model
moves in step. Every face's normal is worked out again from where its corners are, so a
face that turns is lit as it faces. It reads the rate as 16.16 fixed point, as for an
animated texture, and uses it as seconds from one frame to the next: 1 s for the
dinosaur, 0.5 s for the pump jack. What is solid is the first frame. `src/pod/mod.rs`
reads the first frame into `Track::models` and every frame's vertices into
`Track::animated_models`; a frame that is missing, or has another vertex count, leaves the
model still. The scenery moves the mesh (`scenery/animation.rs`).

## Placing a model in a track

A box in the track file gives a position (the model's origin) and a heading `psi`. No
object in `AlpineMtns.pod` has any pitch or roll, so those are **open**.

**Measured:** the model's Z axis points along the heading, (sin psi, cos psi) on the
ground, and its X axis along (cos psi, -sin psi). Guard rails are long in X and laid in
lines: under this rule 29 of 33 point at their nearest neighbour to within 18 degrees,
and under the opposite sign 13 do. Checkpoint banners are long in X too, and under this
rule all nine lie across the course. In the game's axes this is the same yaw of `-psi`
as a truck's.

A checkpoint is as wide as its model: 110 ft for Alpine's banners, 192 ft for its finish
line, whose model `CKBOX.BIN` is a plain 192 x 32 x 32 ft box. That box is the editor's
trigger volume (its texture is the arrows that track makers line up with the direction
of travel), and the game does not draw it. That it is invisible in MTM2 too, and that
names starting `CKBOX` are the way to tell, is **reference** (community documentation).

## Backdrops

The models that the track file's Backdrop section names (situation.md).

- **Measured** on the 9 in our archives (`tests/pod_real_tracks.rs`): each is a ring of
  8 to 32 upright faces round its origin, 163 to 202 ft out, reaching 28 to 87 ft below
  the origin and 28 to 51 ft above it. Every face is a cutout (type 17): the black of the
  texture is the sky.
- The corners of some models run one way round and of others the other (Alpine's
  `11EDROP.BIN` and Hang Time's `C3DROP.BIN`), so the game draws both sides.
- **Reference** (JSTrackViewer, `src/scene.js`, citing the Traxx editor's source): the
  original game draws the backdrop centred on the camera every frame, so that it never
  comes nearer, first and with no depth test, so that everything else is drawn over it,
  without lighting or fog, and at the size it was made, without the height stretch that
  objects get. The game here keeps it centred on the camera and scales it up to just inside
  the camera's far plane, which looks the same from its middle (`src/backdrop.rs`).
- A backdrop's textures are larger than the scenery's (256 x 256 in 6 of the 9) and have
  palettes of their own: see textures.md.

## Open

- Which box types are solid. The game makes everything solid except types 6
  (checkpoint), 7 and 8 (**reference**: "drive through" and "always face the camera").
  Type 8 turns to the camera: see situation.md.
- Objects with a mass (91 of Alpine's 323, mostly signs) can presumably be knocked over.
  The game fixes everything in place.
- How MTM2 shows an animated texture and moves a keyframed model, and what their rates
  mean. The game's own rules are above.
- The ints of a texture cycle and an animation control file that are 0 in every file.
- What the Order record's numbers mean, and whether any model has faces that a Jump jumps
  over.
