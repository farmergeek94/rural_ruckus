# Contributing

How to build, run, test and inspect the game. `AGENTS.md` holds the short rules.

## Commands

```sh
cargo run                          # the front end: choose a truck, a track and a setup, and GO
cargo run -- --builtin             # the front end with the built-in truck and track listed too (development only)
cargo run -- --race --builtin      # straight to a race in the built-in truck on the built-in track
cargo run -- tracks/MyTrack.pod --builtin             # an MTM2 track in the built-in truck
cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod   # in one of its trucks (either order)
cargo run -- tracks/MyTrack.pod --base-game=/path/to/MTM2   # the base game's archives from this folder, this run only
cargo run -- tracks/AlpineMtns.pod --opponents=7      # against seven computer trucks
cargo run -- tracks/AlpineMtns.pod --weather=storm    # clear, overcast, fog, rain, storm, snow or random (F7 in a race)
cargo run -- tracks/AlpineMtns.pod --smooth-terrain   # the ground lit as if rounded off
cargo run -- tracks/AlpineMtns.pod --no-blend-ground  # ground textures edge to edge, as MTM2 draws them
cargo run -- tracks/AlpineMtns.pod --no-mipmaps       # textures without mipmaps, to compare
cargo run -- tracks/AlpineMtns.pod --no-backdrop      # without the distant hills round the horizon
cargo run -- --base-game=$HOME/Documents/mtm2 --no-settle-scenery   # objects on slopes stand as the track puts them, feet floating
cargo run -- tracks/AlpineMtns.pod --integrated-graphics   # graphics for a processor's built-in GPU (INTEGRATED GRAPHICS on the options screen)
cargo run -- tracks/AlpineMtns.pod --antialiasing=fxaa --shadow-cascades=2 --shadow-distance=100 --anisotropy=4   # single cheaper settings (F2 changes them in a race)
cargo run -- --log-fps --no-vsync  # print the true frame time once a second
cargo run -- --fifo                # strict vsync (PresentMode::Fifo): never shows a late frame early
cargo run -- tracks/AlpineMtns.pod --log-fps --autopilot   # measure while it drives itself
cargo test                         # all tests (headless, no window needed)
cargo test --test track            # one integration test file
cargo test --test camera_smoothness -- --nocapture   # the camera's rules, printing every smoothness figure
cargo test -- --ignored            # slow tests too, and ones that print figures
cargo clippy --all-targets         # lint
cargo fmt                          # format
```

## Real files

MTM2's files are copyrighted and are never committed. Git ignores `tracks/`, `trucks/`
and `base/` (see the README for what goes where). `base/` is searched with its
subfolders, so links to an MTM2 CD's `Shared` and `English` (or other language) folders
will do.

The game finds the base game's archives, which tracks and trucks borrow from, in this
order:

1. `--base-game=FOLDER` on the command line. Not kept.
2. The two folders that the player chose in the front end: `Shared`, and one language
   folder (`English`, `French` or `German`). Kept in `saves/ruckus.redb`.
3. `base/`.

On the CD the language folders are beside `Shared` and hold archives with the same
names, so the player chooses one language folder, not the CD root. Only a folder that
holds archives directly can be chosen. The front end asks for each folder when it opens
if nothing above gives one, and OPTIONS > FILES changes them. Without the base game, what
tracks and trucks borrow is missing: built-in shapes and grey in its place.

## Keys in a race

| Key | Does |
| --- | --- |
| W, S, A, D (or the arrows) | Throttle, brake and reverse, steer |
| Space | Handbrake |
| Left Shift, Left Ctrl | Shift up, shift down, with manual gears (OPTIONS > Gears, or `--manual`) |
| R | Flip the truck upright |
| C | Back to the last checkpoint |
| Esc | Pause: continue, restart the whole race, save a screenshot (in `screenshots/`), or cancel the race |
| F1 | The physics colliders' wireframes |
| F2 | Graphics panel; F3 to F6 step its settings |
| F7 | Next weather |
| F8 | Next time of day |
| V, Q, E, B | Change view; look left, right and back |
| M | Map of the track |

## Build profile

The dev profile compiles this crate unoptimised (for the debugger) and its dependencies
at `opt-level = 3`, so dev builds are playable. Do not change that, and do not use
`--release` to test or run the game, unless the user asks. The reference figures in
[docs/smoothness.md](docs/smoothness.md) are for dev builds.

`.vscode/launch.json` holds CodeLLDB configurations for the game and for each test file.
Add one when you add a test file.

## Tests

- **Pure logic** gets unit tests in a `#[cfg(test)] mod tests` at the bottom of its file
  (see `race/gate.rs`). Keep rules and geometry as plain functions over plain data, and
  the systems round them thin.
- **Physics behaviour** (how the truck drives, the race, the computer's drivers, the
  camera behind a truck) is checked by driving it. The user had the headless tests that
  simulated it removed, because tuning kept breaking them. Do not add them back unless
  the user asks. A probe that measures something headless while you work is fine; delete
  it afterwards.
- **Other behaviour in the running game** (the front end, the game state, the graphics
  panel, POD conversion) gets headless integration tests in `tests/`, one file per slice,
  named after it: `MinimalPlugins` plus the real game plugins, so plugins must work
  without a window or renderer. The modules that know nothing of the game have theirs
  beside them (`store.rs`, `camera_smoothness.rs`). A slice whose behaviour is physics
  (`truck`, `race`, `opponents`, `camera`) has none. Mark a test that simulates more than
  a few seconds `#[ignore]`, with a reason.
- **No committed test reads `tracks/`, `trucks/` or `base/`.** Measure real files with a
  probe in the scratch directory, record the result in `docs/formats/`, and delete the
  probe (see AGENTS.md).
- Bevy keeps resources as entities too: a test that counts a race's entities
  (`tests/game_state.rs`) counts `Without<IsResource>`.

## Looking at a screen without a window

A Bevy app with no window can draw to an image and save it. Give the camera a
`RenderTarget::Image` and `IsDefaultUiCamera` (with no window there is no default camera
for the UI). Use this instead of taking over the user's desktop.

## Measuring

`--log-fps` and `--autopilot` (`src/diagnostics.rs`) measure frame time and smoothness.
What the figures mean, and the reference figures, are in
[docs/smoothness.md](docs/smoothness.md).
