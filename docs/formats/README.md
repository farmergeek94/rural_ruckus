# Monster Truck Madness 2 file formats

What this project knows about MTM2's files, and how it knows it. The formats were never
published, so every fact is marked with where it came from:

- **Measured**: read out of a real file. The document names the files and the figures,
  so that anyone with the files can measure them again. No committed test holds them,
  because the files may not be there.
- **Reference**: from one of three open-source projects by the same author. The fact
  agrees with our files, but we did not prove it independently. A reference finding
  names its source:
  - [JSPod](https://github.com/juanputrerasm/JSPod): the `.POD` archive and its files.
  - [JSTrackViewer](https://github.com/juanputrerasm/JSTrackViewer) (Apache 2.0): a
    viewer for Terminal Reality tracks.
  - [JSTruckViewer](https://github.com/juanputrerasm/JSTruckViewer): a viewer for
    Terminal Reality trucks.
- **Open**: not known yet. Do not fill these in with guesses.

| File | Format | Parser |
| --- | --- | --- |
| [pod.md](pod.md) | The `.POD` archive | `crates/pod/src/archive.rs` |
| [level.md](level.md) | `.LVL`, the list of a track's files | `crates/pod/src/level.rs` |
| [terrain.md](terrain.md) | `.RAW` heightmap and `.CLR` texture map | `heightmap.rs`, `texture_map.rs` |
| [textures.md](textures.md) | `.TEX` list, `.RAW` textures, `.ACT` palette, and how a texture lies on a cell | `crates/pod/src/textures.rs`, `texture_map.rs` |
| [texture_types.md](texture_types.md) | `.TTY`, the type of surface of each texture: ice | `crates/pod/src/texture_types.rs` |
| [ground_boxes.md](ground_boxes.md) | `.RA0`, `.RA1` and `.CL0`, the solid blocks of bridges and tunnel roofs | `crates/pod/src/ground_boxes.rs` |
| [model.md](model.md) | `.BIN` 3D models, and how they are placed | `crates/pod/src/model.rs` |
| [situation.md](situation.md) | `.SIT` and `.SI2`, the track definition | `crates/pod/src/situation.rs` |
| [truck.md](truck.md) | `.TRK`, the truck definition, and a truck's models and textures | `crates/pod/src/truck_file.rs` |
| [cockpit.md](cockpit.md) | `POWERBIG.480`, the dashboard's layout, and its pictures | `crates/pod/src/cockpit.rs`, `textures.rs` |
| [sound.md](sound.md) | `.WAV` sounds: a truck's three, and the engine's loops | `crates/pod/src/wave.rs` |
| [conventions.md](conventions.md) | Units and axes, and how the game converts them | `crates/pod/src/track/mod.rs`, `crates/pod/src/truck.rs` |

Real archives are copyrighted and never committed: tracks go in `tracks/` and trucks in
`trucks/`, which git ignores. No committed test needs them: measure them with a probe in
the scratch directory (see AGENTS.md).

Archives examined first:

| Archive | What it is | Contents |
| --- | --- | --- |
| `MyTrack.pod` | "Tracked2 School by Phineus", a flat community tutorial circuit | 20 files. Terrain and track only: its textures and models are in the base game's archives. |
| `99BFoot.pod`, `VirginiaGiant2003.pod` | Two community trucks | See [truck.md](truck.md). |
| `AlpineMtns.pod` | "Alpine Mountains", a steep mountain circuit with 10 checkpoints and 323 scenery objects | 366 files. Self-contained: 300 ground and model textures (`ART\*.RAW`, 4096 bytes, 64 x 64 at 8 bits), 5 palettes (`.ACT`), 32 models (`MODELS\*.BIN`), music and UI art. |

The base game's own archives: see [pod.md](pod.md).
