# Units and axes

| | MTM2 | This game |
| --- | --- | --- |
| Length | feet | metres (x 0.3048) |
| Height | 2 ft per heightmap step; positions in feet | metres |
| Origin | a corner of the 8192 ft world | the centre of the world |
| Up | Y | Y |
| Horizontal axes | X and Z, left-handed | X and Z, right-handed: **Z is flipped** |
| Heading | `psi` radians from +Z | yaw about Y from -Z, equal to `-psi` |

A position (x, y, z) in feet becomes ((x - 4096) x 0.3048, y x 0.3048, (4096 - z) x 0.3048).

Feet and the height step are **measured** (terrain.md). The Z flip is **measured** too:
the lettering on a truck's sides reads left to right only if the files' axes are
left-handed, X to the right and Z forwards with Y up (truck.md). A right-handed scene
must flip one axis to show them, and flipping Z turns a heading into `-psi`. It agrees
with `MyTrack.pod`, whose converted circuit starts northwards with the far checkpoint to
the west, and with JSTrackViewer, which does the same.

The conversion happens once for a track, in `crates/pod/src/track/mod.rs`, and once for a
truck, in `crates/pod/src/truck.rs`. The `pod` crate's parsers keep MTM2's own units
and axes so that they stay a faithful description of the files.
