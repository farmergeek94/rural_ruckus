# Smooth motion and performance

What was measured about judder, lurching, tearing and frame time, and the rules that
come from it. Read this before you change the camera, the clock, how the truck is drawn,
or how large geometry is built. `AGENTS.md` holds the short rules.

## Judge smoothness by the `view:` line, not by frame rate

`--log-fps --autopilot`
reports how much the view's turn changes from one frame to the next, which is what the
eye sees as judder: a tenth of a degree is a few pixels of whole-screen shake. Figures
under vsync, driving Alpine and MyTrack (median, 95th percentile, worst, in degrees):

| | Median | 95th | Worst |
| --- | --- | --- | --- |
| Truck drawn at its physics pose, raw clock | 0.03 to 0.05 | 0.10 to 0.17 | 0.3 |
| Interpolated truck, raw clock | 0.03 | 0.11 to 0.13 | 0.3 to 0.6 |
| Interpolated truck, first clock steadier (20 frames, since replaced) | 0.008 | 0.03 | 0.06 |
| Ideal: exactly two physics steps forced into every frame | 0.008 | 0.03 | 0.06 |

Smooth motion needs **both**: what is drawn must be interpolated between physics steps
(`truck/interpolate.rs`), and the clock must be paced (`frame_pacing.rs`). Under vsync
the screen shows every frame for one refresh, but the moment a frame starts, which is
all a clock can measure, is early or late: on the development machine by 4 to 13 ms for
a quarter of all frames. A clock that believes that places everything unevenly. The
plainest demonstration is the truck held in a circle at 62 deg/s on MyTrack, with the
frame lengths from a real log (measured headless, judder on screen in degrees, median /
95th / worst):

| | |
| --- | --- |
| Every frame the same length | 0.000 / 0.000 / 0.000 |
| The logged frames, raw clock | 0.44 / 1.24 / 1.24 |
| The logged frames, paced (as shipped) | 0.000 / 0.001 / 0.001 |

**How the clock got here**, so that it isn't gone round again. The first steadier
averaged twenty frames and passed anything over 1.75 times the average on whole, as a
stall. Twenty is too few: the error of a mean is the lateness of its first and last
frames over its length, so the step wandered by 4% from frame to frame. And 1.75 is
inside what a late frame reaches on this machine (30 ms against 16.7), so ordinary
frames were passed on whole and the averaging begun again: the camera covered 0.53 m
in a frame instead of 0.31 m, a jerk every few seconds. Raising the factor to 3 cured
the jerk and left the wander, which looked like everything speeding up and slowing
down. It was then taken out altogether, and the raw clock's shake is the table above.
`Pacer` takes the mean of sixty frames, smooths that, feeds back a hundredth of the
real time still owed, and calls a stall four refreshes. It does not make up a missed
refresh with a double step: nothing in one measurement tells a missed refresh from a
late start, a guess that is wrong is a jump, and being sure takes a third of a second,
by when a double step is a second hitch. Candidates were tried against simulated
frame times before any was written; do the same before changing it, and add the case
to its tests. Not paced without vsync, where frames are shown as they are finished and
the measurements are the truth. Vsync is `PresentMode::AutoVsync`, which prefers
`FifoRelaxed`: a frame that just missed a refresh is shown at once, which can tear the
top of the picture where frames start late. `--fifo` asks for `PresentMode::Fifo`, which
holds it for the next refresh and never tears. Both are paced. The window's `desired_maximum_frame_latency` of 1 went
with the first steadier and has not come back. **The pacer has not yet been measured
on a real screen**: `--log-fps --autopilot` and the `view:` line, against the first
table. Any change to the camera, the clock or how the truck is drawn must be
re-measured. A genuine stall (one frame of 300 ms) shows as a single large "worst" with
a matching `frames:` line; repeat a measurement before believing it.

## Judder and lurching are different, and need different measurements

The `view:`
line catches shake between consecutive frames. It is blind to the view's rate of turn
swinging about a few times a second, which is what a bump or a tap of the steering
does through a camera that aims rigidly at the truck, and which looks just as "jerky".
Measured headless on Alpine with the autopilot, as the change in the view's rate of
turn between tenths of a second (deg/s, 95th percentile / worst):

| | Pitch | Yaw |
| --- | --- | --- |
| Camera aimed straight at the truck, hung off its heading | 2.08 / 4.07 | 10.0 / 20.3 |
| Eye, heading and aim each drawn towards their places by a plain lag, instant key steering | 0.38 / 0.82 | 4.8 / 9.3 |
| The same, with today's ramped key steering (measured headless, before the rewrite) | 0.32 / 0.70 | 11.6 / 15.4 |
| Today's camera (`camera/chase.rs`), ramped key steering | 0 / 0 | 9.9 / 13.3 |

The ramped keys make the autopilot weave, which is most of the yaw figure whatever the
camera: compare rows measured with the same steering. The same minute of driving, for
the old camera and the new (median / 95th / worst unless said):

| Alpine, autopilot, even 60 Hz | Old camera | Today's camera |
| --- | --- | --- |
| Judder (deg) | 0.012 / 0.032 / 0.042 | 0.010 / 0.026 / 0.036 |
| Truck off centre (deg) | 0.87 / 3.73 / 4.74 | 0.29 / 1.50 / 3.17 |
| of which sideways | not measured | 0.08 / 0.25 / 0.43 |
| Eye to truck (m), least to most | 15.3 to 17.7 | 13.3 to 13.7 |

**Nothing is fastened to the truck, and nothing trails it.** A plain lag
(`1 - exp(-k * dt)`) trails a moving target by `speed / k`: the old camera's truck sat
off centre by an amount that changed with speed, and its eye, smoothed separately, was
`13 + speed / 4` metres away, which looked like the camera falling back and catching
up. Today's camera smooths two things, each with a critically damped spring that works
on the gap to a *moving* target and so follows a steady one with no lag: the aim (tight
over the ground, loose in height, so that a body bouncing on its springs moves in the
picture rather than the picture) and the heading it sits behind (loose, so that a tap of
the steering doesn't swing it). The eye is placed from those two, not smoothed, so its
distance is constant and the picture never pitches. The target's vertical speed is only
gone along with as it lasts, and let go of at once when it ends: taken up fully, the
camera rides every bounce; held on to, it carries on down after a truck that has landed
(0.46 m past, measured). Tune against `tests/camera_smoothness.rs`, whose figures
are budgets, and then by driving.

## Tearing at the top of the screen is the desktop's, not the game's

On X11 a frame can
only be swapped in cleanly (a page flip) when the window covers the whole X screen. With
several monitors making up one X screen, a fullscreen window on one of them never does,
so X copies each frame in at the refresh, just as the monitor starts drawing from the
top, and the top of the picture tears, on any GPU and in any present mode. Found on the
development machine (four monitors, Cinnamon, no `TearFree` in its X driver), where
frames drawn to images were identical from one to the next, the Intel and NVIDIA GPUs
tore alike, the compositor cured it at the price of jerkiness, and turning the other
monitors off cured it outright. Check that (`xrandr --output ... --off`) before looking
for a cause in the game.

## A problem you cannot see

When the user reports a problem you cannot see, **find a number that shows it before
you change anything**. Do not launch the game on their desktop to do it while they use
the machine: measure headless where you can.

## Never draw or follow a physics body directly

Draw a separate entity placed
between the last two physics poses (`TruckVisual` follows `Truck`), and order systems
that follow it `.after(TruckSystems::PlaceVisuals)`. Do not move the body itself for
looks (the physics takes that for a teleport), and do not add Avian's
`TransformInterpolation` to a body: it moves the body's own `Transform` between steps,
which gameplay (race progress, tests) reads. `TruckVisual` is what is interpolated.

## Measure performance; don't guess

`--log-fps --no-vsync` shows the real frame time
(with vsync on, everything under 16.7 ms reads as 60 fps). Measure while driving as
well as parked: the budget at 60 Hz is 16.7 ms and a frame that misses it is a visible
stutter. Reference figures on the development machine (integrated GPU, debug build),
parked at Alpine's start: about 9.5 ms, of which the sun's shadows are about 2 ms.
The INTEGRATED GRAPHICS preset (`front_end::integrated_graphics`, `--integrated-graphics`)
turns down what costs most on such a GPU; its values are chosen, not measured, so measure
it the same way before changing them.

## Culling is per entity, and automatic

Bevy skips any mesh entity whose bounding box
is outside the view (and outside the sun's shadow range), so anything that is its own
entity, like each scenery object, is already culled. Check with `--log-fps`, which
prints `meshes_in_view`: parked at Alpine's start, 104 of 583 meshes and 34 of 322
scenery objects are drawn. What defeats culling is one big entity.

## Large static geometry goes in chunks

Put each chunk in one mesh, so that Bevy's culling can
skip what is off screen or beyond the shadows' reach (`track/mesh.rs`, 16 x 16 ground
cells per chunk, for shaded ground as for textured). A single mesh is drawn in full for the view and again for every
shadow cascade. To make the ground look smoother, change how it is lit, not its shape:
a finer height grid for the physics as well cost a debug build half its frame rate with
eight trucks, most of it in the wheels' casts onto the ground (`track/shading.rs`).

## Only what has holes is alpha-masked

A material with an alpha mask (`AlphaMode::Mask`) may leave pixels out, so the GPU cannot
reject a pixel that is hidden before it shades it, and each of the sun's shadow cascades
draws the mesh with a fragment shader instead of by its depth alone. So the scenery has
two materials on the one texture array (`scenery/mod.rs`): an opaque one for the models
whose tiles have no texel under the mask's cutoff, which draws exactly as the mask did,
since filtering and mipmaps only mix texels, and a masked one for the rest. The masked
one's shadows are cut out by `track/tiles_prepass.wgsl`: Bevy's own prepass shader reads
the standard material's texture, which the tile material has not got, so a cut-out tile
cast the shadow of its whole square. **Not yet measured on screen**: the F2 panel with
`--no-vsync`, on a track with much scenery, with the shadow cascades at 4 and at 0.

## Mipmaps

Generate mipmaps on the CPU, averaging in linear light, for textures known at load
time. Bevy's GPU mip generator handles neither sRGB formats nor texture arrays, and
is meant for images rendered at run time. Never pack tiles that are sampled with
mipmaps into an atlas: use a texture array, whose layers cannot bleed into each other.
