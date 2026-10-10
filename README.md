# Monster Truck Rural Ruckus

A modern replacement for *Monster Truck Madness 2* (MTM2, Terminal Reality / Microsoft,
1998), written in Rust on Bevy and Avian. Trucks race laps around a track and must pass
its checkpoints in order. The game loads MTM2's `.POD` archives directly: the original
tracks and trucks, and community-made ones.

This repository contains no MTM2 files. You must supply them from your own copy of the
game.

![A race in the desert: Bigfoot follows two trucks along a dirt course, with dust behind the wheels](docs/images/desert.jpg)

![Eight trucks on the starting grid at night, in rain, with headlights on](docs/images/rain-at-night.jpg)

The screenshots show MTM2 tracks and trucks, from the original game's files and from
community additions.

## Acknowledgements

The reverse-engineering work by [Juan Pablo Utreras](https://github.com/juanputrerasm)
is a major source of knowledge for this project, especially on MTM2's POD archives and
file formats.

## 1. Install the tools

1. Install Rust with [rustup](https://rustup.rs). Bevy 0.19 needs a recent stable Rust
   (`rustup update`).
2. On Linux, install the libraries that Bevy needs. On Debian or Ubuntu:

   ```sh
   sudo apt install g++ pkg-config libx11-dev libasound2-dev libudev-dev libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev
   ```

   For other distributions, see Bevy's
   [Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).

## 2. Supply the game files

Put your files in these folders. Git ignores all three.

| Folder | Put in it |
| --- | --- |
| `tracks/` | Track archives (`.pod`) |
| `trucks/` | Truck archives (`.pod`) |
| `base/` | The base game's archives, or links to the `Shared` and `English` folders of your MTM2 CD |

Tracks and trucks borrow textures and models from the base game's archives. To link the
CD folders:

```sh
mkdir -p base
ln -s "/media/<you>/MTM2/Shared"  base/Shared
ln -s "/media/<you>/MTM2/English" base/English
```

You can also choose these folders in the game, at OPTIONS > FILES. Without the base game
the game still runs, but borrowed parts show as plain grey shapes.

## 3. Start the game

```sh
cargo run
```

The first build takes some minutes. Then the front end opens. Choose a truck, a track and
a setup, and select GO.

To go directly to a race, name a track and a truck (in either order), with options:

```sh
cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod
cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod --opponents=7     # seven computer trucks
cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod --weather=rain    # clear, overcast, fog, rain, storm, snow or random
cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod --time=night      # day, dusk or night
cargo run -- --base-game=/path/to/MTM2                               # the base game from this folder, this run only
cargo run -- --race --builtin                                        # the built-in truck and track (no files needed)
```

[CONTRIBUTING.md](CONTRIBUTING.md) lists all the options.

### Release build

For the best frame rate, use the release profile. It takes longer to build, because it
optimises all of the code together.

```sh
cargo run --release
./target/release/monster_truck_rural_ruckus   # start it without Cargo
```

Start it from the repository folder: the game finds `tracks/`, `trucks/`, `base/` and
`saves/` relative to the current folder.

If you change the code, test with the dev profile. The debugger shows more in a dev
build, and the reference figures in [docs/smoothness.md](docs/smoothness.md) are for it.

## 4. Drive

The keys are in [CONTRIBUTING.md](CONTRIBUTING.md#keys-in-a-race). In short: W, S, A, D
or the arrows drive, Space is the handbrake, R flips the truck upright, C goes back to
the last checkpoint, and Esc pauses.

## More information

| Document | Contents |
| --- | --- |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Build, run and test commands, all options, keys, test policy |
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the code is organised |
| [docs/roadmap.md](docs/roadmap.md) | The roadmap |
| [docs/formats/](docs/formats/README.md) | What is known about MTM2's file formats |
