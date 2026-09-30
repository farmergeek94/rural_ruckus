# Contributing

How to build, run, test and inspect the game. `AGENTS.md` holds the short rules.

## Commands

```sh
cargo run                          # the front end: choose a truck, a track and a setup, and GO
cargo run -- --builtin             # the front end with the built-in truck and track listed too (development only)
cargo run -- --race --builtin      # straight to a race in the built-in truck on the built-in track
cargo run -- tracks/MyTrack.pod --builtin   # play a Monster Truck Madness 2 track in the built-in truck
cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod   # in one of its trucks (either order)
cargo run -- tracks/AlpineMtns.pod --smooth-terrain   # with the ground lit as if rounded off
cargo run -- tracks/AlpineMtns.pod --no-mipmaps       # textures without mipmaps, to compare
cargo run -- --log-fps --no-vsync  # print the true frame time once a second
cargo run -- --fifo                # strict vsync (PresentMode::Fifo): never shows a late frame early
cargo run -- tracks/AlpineMtns.pod --integrated-graphics   # start with the graphics for a processor's built-in graphics (INTEGRATED GRAPHICS on the options screen)
cargo run -- tracks/AlpineMtns.pod --antialiasing=fxaa --shadow-cascades=2 --shadow-distance=100 --anisotropy=4   # start with single cheaper graphics settings (F2 changes them while racing)
cargo run -- tracks/AlpineMtns.pod --log-fps --autopilot   # measure while it drives itself
cargo run -- tracks/AlpineMtns.pod --opponents=7   # against seven computer trucks
cargo run -- tracks/AlpineMtns.pod --no-backdrop   # without the distant hills round the horizon
cargo run -- tracks/MyTrack.pod --base-game=/path/to/MTM2   # the base game's archives from this folder, for this run only
cargo run -- tracks/AlpineMtns.pod --weather=storm  # clear, overcast, fog, rain, storm, snow or random (F7 changes it while racing)
cargo test                         # all tests (headless, no window needed)
cargo test --test base_game -- --ignored --nocapture   # what each track and truck borrows from the base game
cargo test --test camera_smoothness -- --nocapture   # the camera's rules, printing every smoothness figure
cargo test --test track            # one integration test file
cargo test -- --ignored            # slow tests too, and ones that print figures
cargo clippy --all-targets         # lint
cargo fmt                          # format
```

## Real files

MTM2's files are copyrighted and are never committed. Git ignores these folders:

| Folder | What |
| --- | --- |
| `tracks/` | Track archives |
| `trucks/` | Truck archives |
| `base/` | The base game's archives, for the tests, and for the game until a folder is chosen in the front end. Its subfolders are searched too, so links to an MTM2 CD's `Shared` and `English` (or other language) folders will do: `ln -s "/media/.../Shared" base/Shared`. |

The base game's archives are the ones that tracks and trucks borrow from. The game finds
them in this order:

1. `--base-game=FOLDER` on the command line. The game does not keep this folder.
2. The two folders that the player chose in the front end: `Shared`, and the folder of one
   language (`English`, `French` or `German`). The game keeps them in
   `saves/ruckus.redb`.
3. `base/`.

On the MTM2 CD, the language folders are beside `Shared` and contain archives with the
same names. Thus the player chooses one language folder, not the CD root. A folder is
available to choose only if it contains archives directly.

The front end asks for each folder when it opens, if no `--base-game` is given and item 2
does not give that folder with archives in it. The player can choose different folders at
OPTIONS > FILES.

Without the base game, the game still runs, and what tracks and trucks borrow is missing:
built-in shapes and grey in its place.

## Keys in a race

| Key | Does |
| --- | --- |
| W, S, A, D (or the arrows) | Throttle, brake and reverse, steer |
| Space | Handbrake |
| R | Flip the truck upright |
| C | Back to the last checkpoint |
| Esc | Pause: continue, restart the whole race, save a screenshot (in `screenshots/`), or cancel the race |
| F1 | The physics colliders' wireframes |
| F2 | Graphics panel; F3 to F6 step its settings |
| F7 | Next weather |

## Build profile

The dev profile compiles this crate unoptimised (for the debugger) and its dependencies
at `opt-level = 3`, so debug builds are playable. Do not change that unless the user asks.

Do not use `--release` when you test or run the game. Use the dev profile. The reference
figures in [docs/smoothness.md](docs/smoothness.md) are for debug builds.

`.vscode/launch.json` holds CodeLLDB configurations for the game and for each test file.
Add one when you add a test file.

## Tests

- **Pure logic** gets unit tests in a `#[cfg(test)] mod tests` at the bottom of its file
  (see `race/gate.rs`). Keep rules and geometry as plain functions over plain data, and
  keep the systems around them thin.
- **Behaviour in the running game that is physics** (how the truck drives, the race, the
  computer's drivers, the camera behind a truck) is checked by driving it. The user had
  the headless tests that simulated it removed, because tuning kept breaking them. Do not
  add them back unless the user asks. A probe that measures something headless while you
  work is fine. Delete it afterwards.
- **Other behaviour in the running game** (the front end, the game state, the graphics
  panel, POD conversion) gets headless integration tests, one file per slice:
  `MinimalPlugins` plus the real game plugins. Plugins must therefore work without a
  window or renderer. Mark a test that simulates more than a few seconds `#[ignore]`,
  with a reason.
- `tests/` has one integration test file per slice, named after it. The modules that know
  nothing of the game have theirs beside them (`store.rs`, `camera_smoothness.rs`,
  `pod_real_tracks.rs`, `pod_real_trucks.rs`). A slice whose behaviour is physics
  (`truck`, `race`, `opponents`, `camera`) has none.
- **Real-file tests** (`tests/pod_real_tracks.rs`, `tests/pod_real_trucks.rs`,
  `tests/pod_track.rs`, `tests/pod_truck.rs`, `tests/base_game.rs`) read `tracks/`,
  `trucks/` and `base/`, and pass trivially when those folders are empty. All but
  `base_game.rs` load without the base game, as their figures were measured that way. `tests/pod_track.rs` can paint a map of the
  ground.
- A race's entities are counted by `tests/game_state.rs`. Bevy keeps resources as
  entities too: count `Without<IsResource>`.

## Looking at a screen without a window

A Bevy app with no window can draw to an image and save it. Give the camera a
`RenderTarget::Image` and `IsDefaultUiCamera`: with no window there is no default camera
for the UI to go to. Use this to look at a screen without taking over the user's desktop.

## Measuring

`--log-fps` and `--autopilot` (`src/diagnostics.rs`) are the tools for frame time and
smoothness. What the figures mean, and the figures to compare against, are in
[docs/smoothness.md](docs/smoothness.md).
