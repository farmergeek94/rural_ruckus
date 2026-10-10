# Sounds

MTM2's sounds are `.WAV` files in the base game's archives. `src/pod/wave.rs` reads them.

## The files

**Measured**, on an English MTM2 CD (the Community Patch's archives in
`~/Documents/mtm2`):

- `English\SOUND.POD` holds 506 `.WAV` files. `English\UI.POD` holds the front end's,
  `Shared\MUSIC.POD` the music, and `English\TRUCK2.POD` a copy of each base truck's
  three sounds (see below).
- Every `.WAV` examined is a RIFF file of uncompressed PCM (format 1), one channel,
  8 bits a sample (unsigned, silence at 128), at 11 025 or 22 050 samples a second. Some
  have `LIST` and `fact` chunks after the `data` chunk. No file has a `smpl` chunk: no
  file marks its own loop.

## A truck's three sounds

The `.TRK` line `Wave File` is followed by three file names (see [truck.md](truck.md)).
**Measured** in the 20 base trucks (`TRUCK2.POD`) and 13 community trucks:

- The three names are a stem and `F`, `U`, `D`: `bfootf.wav`, `bfootu.wav`,
  `bfootd.wav`. The stem is the truck's (`bear`, `bfoot`, `crush`, `dozer`, `monst` and so
  on). Community trucks borrow a base truck's stem: 10 of the 13 use `bfoot`, two `monst`
  and one `CRUSH`.
- `EXECUTE.TRK` names `exef.wav` twice, as its first and its third sound. There is no
  `EXED.WAV` in any archive.
- Only one community archive (`MAXD.POD`) holds the files itself. The others rely on the
  base game's copies.
- Each is 0.6 to 1.9 s long, at 11 025 samples a second. The loudness falls nearly to
  silence part-way through most of them (`BFOOTF.WAV`: a root mean square of 2 out of 128
  in its fourth eighth, between 25 and 55 on either side), and the pitch does not rise or
  fall steadily through them. They are not engine loops. `SOUND.POD` also holds
  `BIGFOOT.WAV`, `BIGFOOT1.WAV` and `BIGFOOT2.WAV`, and the like for the other base trucks.

**Measured** by ear (the user listened to `BFOOTF.WAV`, `BFOOTU.WAV` and `BFOOTD.WAV`):
each is a voice saying the truck's name. `F` says it high, `U` at a medium pitch, and `D`
slowly. When the game plays each is **open**.

## The engine

The engine's sounds are not the truck's: no `.TRK` names them, and `SOUND.POD` has one
set. **Measured** in `SOUND.POD`, all at 22 050 samples a second:

| File | Samples | Made of |
| --- | --- | --- |
| `STARTIDL.WAV` | 184 865 | 124 006 samples of start-up, then the idle loop |
| `IDLE2M1.WAV` | 137 841 | the idle loop, a change, then the M1 loop |
| `M1-2-IDL.WAV` | 139 171 | the M1 loop, a change, then the idle loop |
| `M1-2-M2.WAV` | 86 547 | the M1 loop, a change, then the M2 loop |
| `M2-2-M1.WAV` | 84 006 | the M2 loop, a change, then the M1 loop |
| `ACCEL3C.WAV` | 104 873 | loud revving, then the idle loop |

The loops are found as the samples that the files share exactly:

| Loop | Samples | Seconds | Root mean square | Step from its last sample to its first |
| --- | --- | --- | --- | --- |
| Idle | 60 859 | 2.760 | 32.7 | 10 (its steps: 8.2 on average, 111 at most) |
| M1 | 51 502 | 2.336 | 61.6 | 88 (16.9 on average, 141 at most) |
| M2 | 23 736 | 1.077 | 76.5 | 110 (21.3 on average, 177 at most) |

- The idle loop is the last 60 859 samples of `STARTIDL.WAV`, `M1-2-IDL.WAV` and
  `ACCEL3C.WAV`, and the first 60 859 of `IDLE2M1.WAV`.
- The M1 loop is the last 51 502 samples of `IDLE2M1.WAV` and `M2-2-M1.WAV`, and the
  first 51 502 of `M1-2-IDL.WAV` and `M1-2-M2.WAV`.
- The M2 loop is the last 23 736 samples of `M1-2-M2.WAV` (from sample 62 811), and the
  first 23 736 of `M2-2-M1.WAV`.

Each loop's last sample steps to its first by no more than its own samples step to each
other, so it repeats without a click. That the game repeats them is **reference** to
nothing: it is read from how the files are made, each beginning with the loop it leaves
and ending with the loop it goes to.

**Measured** by ear (the user listened): `2NDGEAR.WAV` and `3RDGEAR.WAV` are gear
changes (0.47 and 0.50 s, which fade out). **Measured**: the English, French and German
`SOUND.POD` hold these two and no other file with `GEAR` in its name, so there is no sound
for a fourth or fifth gear. This agrees with three forward gears, but does not prove it.
Of the others, `REVERSE.WAV` is a reversing beep (0.4 s of
sound, then silence, 0.78 s in all), and `DIESEL.WAV` is a diesel engine (1.6 s, at
11 025 samples a second). `CRUISE.WAV` (2.7 s, at 11 025) is not wanted.

Which loop MTM2 plays, by the user's account of the game: M1 is first gear or reverse
pulling away, and M2 every other gear. This is neither measured nor from a cited source.

**Open**: how fast the engine must turn for each loop, whether the game changes their
pitch, and what `ACCEL3B.WAV`, `ACCEL3C.WAV` and `ACCEL6B.WAV` are for. **Measured** by
ear: `ACCEL6C.WAV` is a fast spin-down, with a turbo spinning down.

How the game uses them: `src/sound/` (`loops.rs`) finds the three loops as above, on every
truck. The gear chooses the loop, as the user describes MTM2: idle in neutral and standing
in first or reverse, M1 pulling away in first or reverse, M2 in every other gear. The
engine's speed sets how fast the loop is played, which is the game's own design. The
engine's speed comes from the engine model (`src/truck/engine.rs`): a rotating mass that a
clutch couples to the wheels through the gear. So the pitch climbs from idle as the truck
pulls away with the converter slipping, and comes down over a change of gear as the clutch
brings the engine to the new gear's speed.

## Other sounds a truck may make

`SOUND.POD` holds, among others, `SKIDCEM0` to `3`, `SKIDDIR0` to `3`, `SKID-C1`,
`SKID-D1`, `SKID-G1` and `SNWSKID5` to `7` (skids, by name on concrete, dirt, grass and
snow), `SPINCEM1` to `3`, `SPINDIR1` to `3`, `SPINICE` and `SPINROC1` to `3` (wheel
spin), `SUSPEN1`, `3`, `5`, `6`, `TR-ROLL1` to `4`, `CRASH_01` to `16`, `SPLASH1` and
`TR-HORN`. What each is for is **open**: only their names say it.
