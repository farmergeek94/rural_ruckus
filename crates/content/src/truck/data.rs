//! A truck as plain data. Everything the game knows about a truck comes from a
//! `TruckData`, whether it is the built-in one or was converted from a Monster Truck
//! Madness 2 archive.
//!
//! Only plain types live here: no entities, asset handles or resources.

use std::sync::Arc;

use bevy::prelude::*;

use super::TruckConfig;

#[derive(Clone)]
pub struct TruckData {
    pub name: String,
    pub config: TruckConfig,
    /// Corners of the body, relative to the chassis centre, in metres. The chassis
    /// collider is the convex hull of them. With fewer than four, a plain box is used.
    pub collider_points: Vec<Vec3>,
    /// What to draw. `None` draws the built-in box on four cylinders.
    pub looks: Option<TruckLooks>,
    /// Red, green and blue of the built-in box, which is the body of a truck that brings
    /// none. Changing it tells trucks that look alike apart.
    pub body_color: [u8; 3],
    /// The lamps on the body, which shine when `TruckLamps` says it is dark.
    pub lamps: Vec<TruckLamp>,
    /// What the driver sees round the road from the cockpit. `None` shows the road alone.
    /// Shared, since its pictures are large and every truck of a kind has the same.
    pub dashboard: Option<Arc<Dashboard>>,
}

/// A dashboard, drawn over the whole of a 4:3 screen, through whose see-through pixels the
/// road shows. Positions are in the screen's own pixels (640 x 480 for MTM2's), from its
/// top left, y down.
#[derive(Clone, Debug, PartialEq)]
pub struct Dashboard {
    /// How many pixels across and down the screen is. Whatever its pixels, it is shown
    /// 4:3.
    pub screen: Vec2,
    /// Where the road shows through the picture looking ahead, in the screen's pixels.
    /// `None` where the layout doesn't say.
    pub window: Option<Rect>,
    /// What is round the road looking ahead, left, right and back. `None` for a way that
    /// has none, where the road fills the screen.
    pub views: [Option<DashboardPicture>; 4],
    /// Drawn looking ahead only.
    pub speedometer: Option<Dial>,
    pub tachometer: Option<Dial>,
    pub steering_wheel: Option<SteeringWheel>,
}

/// A picture as red, green, blue and alpha bytes, row by row from the top. Alpha 0 is see
/// through.
#[derive(Clone, Debug, PartialEq)]
pub struct DashboardPicture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A dial whose needle turns clockwise, as far as the reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dial {
    /// Where the needle turns, in the screen's pixels.
    pub center: Vec2,
    /// How long the needle is, in the screen's pixels.
    pub radius: f32,
    /// Where the needle points at a reading of 0, in radians clockwise from straight up.
    pub zero: f32,
    /// How far it turns for each unit of the reading, in radians: per metre a second on a
    /// speedometer, per revolution a minute on a tachometer.
    pub per_unit: f32,
}

/// A steering wheel: one picture for each of several turns.
#[derive(Clone, Debug, PartialEq)]
pub struct SteeringWheel {
    /// Where the pictures go, in the screen's pixels.
    pub rect: Rect,
    /// Each picture, with how far the wheel is turned in it as a share of the most that any
    /// is turned, from -1 (right) to 1 (left), as `TruckInput::steer` goes, in order of
    /// that turn. At least one.
    pub frames: Vec<(f32, DashboardPicture)>,
}

/// A lamp on a truck: a glow at the lamp, and, for some, a beam that lights what is in
/// front of it.
#[derive(Clone, Debug, PartialEq)]
pub struct TruckLamp {
    /// Where it is, in metres, from the chassis centre, in the truck's own axes.
    pub position: Vec3,
    /// Which way it shines, of length 1, in the truck's own axes.
    pub facing: Vec3,
    /// How fast it turns round the truck's up, in radians a second, positive to the left:
    /// a beacon that goes round. 0 for a lamp that stays put.
    pub spin: f32,
    /// How big the glow is, in metres from its middle to its edge.
    pub glow_radius: f32,
    /// The glow's picture: a texture of the truck's `TruckLooks`, added onto what is behind
    /// it, so that black is nothing. `None` draws a plain disc of `color`.
    pub glow: Option<usize>,
    /// The colour of its light, in linear red, green and blue, the brightest of them 1.
    pub color: [f32; 3],
    pub beam: Option<Beam>,
    /// For a lamp that blinks: how long it is on, then off, in seconds.
    pub blink: Option<[f32; 2]>,
}

/// The light a lamp throws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beam {
    /// How far it reaches, in metres.
    pub reach: f32,
    /// How wide it is at the lamp, from its middle to its edge, in metres.
    pub width: f32,
    /// How far it spreads from its middle to its edge, in radians.
    pub spread: f32,
}

impl TruckData {
    /// The truck that needs no files: the one the handling was tuned on.
    pub fn builtin() -> Self {
        Self {
            name: "Built-in truck".into(),
            config: TruckConfig::default(),
            collider_points: Vec::new(),
            looks: None,
            body_color: BUILTIN_BODY_COLOR,
            lamps: builtin_lamps(),
            dashboard: None,
        }
    }

    /// The same truck in colour `number` of a few that are easy to tell apart. Number 0
    /// is the truck's own. Only a truck that is drawn as the built-in box changes.
    pub fn in_color(mut self, number: usize) -> Self {
        self.body_color = BODY_COLORS[number % BODY_COLORS.len()];
        self
    }
}

const BUILTIN_BODY_COLOR: [u8; 3] = [191, 26, 26];

/// Half the size of the built-in box that stands in for a body a truck doesn't bring, in
/// metres: how far it reaches from the chassis centre along each axis.
pub const CHASSIS_HALF_EXTENTS: Vec3 = Vec3::new(1.1, 0.45, 2.3);

/// The built-in truck's lamps: those of a body the size of its box.
fn builtin_lamps() -> Vec<TruckLamp> {
    let half = CHASSIS_HALF_EXTENTS;
    let (headlights, tail_lights) = lamps_for_body(-half, half);
    [headlights, tail_lights].concat()
}

/// The beam most of the base game's headlights have: 75 ft long, 0.7 ft wide at the lamp
/// and 11 ft at its end (see `docs/formats/truck.md`). For a truck that has headlights
/// with no beam, or none at all.
pub fn headlight_beam() -> Beam {
    const METRES_PER_FOOT: f32 = 0.3048;
    Beam {
        reach: 75.0 * METRES_PER_FOOT,
        width: 0.7 * METRES_PER_FOOT,
        spread: (11.0f32 - 0.7).atan2(75.0),
    }
}

/// Which way a headlight shines that the game aims: straight ahead and a little down, as
/// the base game's headlights are (0.17 rad).
pub fn headlight_facing() -> Vec3 {
    Vec3::new(0.0, -0.17f32.sin(), -0.17f32.cos())
}

/// The game's own lamps for a body that reaches from `min` to `max` (in the truck's own
/// axes, the front towards -Z): two white headlights with beams on its front, a little in
/// from its sides and a little over halfway up, and two red lamps on its back. For a truck
/// that brings none.
pub fn lamps_for_body(min: Vec3, max: Vec3) -> (Vec<TruckLamp>, Vec<TruckLamp>) {
    let across = (max.x - min.x) / 2.0;
    let middle = (max.x + min.x) / 2.0;
    let height = |share: f32| min.y + (max.y - min.y) * share;
    let head = |side: f32| TruckLamp {
        position: Vec3::new(middle + side * across * 0.7, height(0.55), min.z),
        facing: headlight_facing(),
        spin: 0.0,
        glow_radius: 0.25,
        glow: None,
        color: [1.0, 1.0, 1.0],
        beam: Some(headlight_beam()),
        blink: None,
    };
    let tail = |side: f32| TruckLamp {
        position: Vec3::new(middle + side * across * 0.8, height(0.6), max.z),
        facing: Vec3::Z,
        glow_radius: 0.15,
        color: [1.0, 0.0, 0.0],
        beam: None,
        ..head(side)
    };
    (vec![head(-1.0), head(1.0)], vec![tail(-1.0), tail(1.0)])
}

/// The built-in red, then colours for the trucks that race against it.
const BODY_COLORS: [[u8; 3]; 8] = [
    BUILTIN_BODY_COLOR,
    [30, 80, 190],
    [235, 190, 30],
    [40, 150, 60],
    [230, 120, 20],
    [130, 50, 170],
    [225, 225, 225],
    [30, 170, 175],
];

/// A truck's models. A truck often borrows parts that aren't in its own archive, and
/// each one missing is drawn as its built-in stand-in, or for the axle not at all.
#[derive(Clone, Default)]
pub struct TruckLooks {
    pub textures: Vec<TruckTexture>,
    /// Relative to the chassis centre.
    pub body: Option<TruckModel>,
    /// Relative to the hub, with the axle along X.
    pub left_tire: Option<TruckModel>,
    pub right_tire: Option<TruckModel>,
    /// Tire models made for one wheel alone, which take the place of the one for its side:
    /// front left, front right, rear left, rear right. Relative to the hub, likewise.
    pub wheel_tires: [Option<TruckModel>; 4],
    /// Relative to the middle of the line between an axle's two hubs.
    pub axle: Option<TruckModel>,
    /// The bars that hold the axles to the body.
    pub axle_bars: AxleLinks,
    /// The shock absorbers, two at each wheel.
    pub shocks: AxleLinks,
    /// The driveshaft, in two halves: from the body to each axle.
    pub driveshaft: AxleLinks,
}

#[derive(Clone)]
pub struct TruckTexture {
    /// Pixels along each side.
    pub size: usize,
    /// Red, green, blue and alpha bytes, row by row from the top. Alpha 0 is a hole.
    pub rgba: Vec<u8>,
    /// Which way the surface faces at each pixel, for a texture that has a normal map.
    pub normal_map: Option<NormalMap>,
}

/// Directions, not colours: each pixel's red, green and blue, from 0 to 255 for -1 to 1,
/// are a direction across the texture, down it, and out of the surface. Alpha is unused.
#[derive(Clone)]
pub struct NormalMap {
    /// Pixels along each side.
    pub size: usize,
    pub rgba: Vec<u8>,
}

/// Straight parts from the body to the axles, all of one kind, drawn as cylinders.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxleLinks {
    /// Empty for a truck without them.
    pub links: Vec<AxleLink>,
    /// In metres.
    pub radius: f32,
    /// Index into `TruckLooks::textures`. `None` draws them in `color` alone.
    pub texture: Option<usize>,
    /// Red, green and blue, where there is no texture.
    pub color: [u8; 3],
}

/// A straight part from a point on the body to a point on an axle, which moves with it: an
/// axle bar, a shock absorber, or half the driveshaft.
#[derive(Clone, Debug, PartialEq)]
pub struct AxleLink {
    /// Where it meets the body, relative to the chassis centre, in metres.
    pub body_end: Vec3,
    /// Which axle: 0 the front one, 1 the rear.
    pub axle: usize,
    /// Where it meets the axle, in metres, from the middle of the line between the
    /// axle's hubs, in the axle's axes: X from the left hub to the right one.
    pub axle_end: Vec3,
}

/// A model in the game's axes and metres, as one mesh for each texture it uses.
#[derive(Clone, Default)]
pub struct TruckModel {
    pub parts: Vec<TruckMesh>,
}

#[derive(Clone, Default)]
pub struct TruckMesh {
    /// Index into `TruckLooks::textures`. `None` is drawn in `color` alone. For an
    /// animated texture, its first frame.
    pub texture: Option<usize>,
    /// For an animated texture: the frames it steps through.
    pub texture_cycle: Option<TruckTextureCycle>,
    /// Red, green and blue of an untextured mesh.
    pub color: [u8; 3],
    /// Whether the texture has holes in it.
    pub cutout: bool,
    /// How opaque the mesh is, from 0 to 1, for one blended with what is behind it, as
    /// glass is. `None` for an opaque mesh or a cutout.
    pub opacity: Option<f32>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Position within the texture, (across, down) from its top left, from 0 to 1.
    pub uvs: Vec<[f32; 2]>,
    /// Three per triangle, counter-clockwise seen from the front.
    pub indices: Vec<u32>,
}

/// An animated texture: textures shown one after the other, round and round, each for the
/// same time, on the game clock.
#[derive(Clone, Debug, PartialEq)]
pub struct TruckTextureCycle {
    /// Indices into `TruckLooks::textures`, in the order they are shown. At least 2.
    pub frames: Vec<usize>,
    /// How long each is shown, in seconds. Greater than 0.
    pub seconds_per_frame: f32,
}
