//! The wind over the water. It blows one way all race, and gusts: it picks up and dies
//! down over a few seconds, never quite still and never a gale. `water.wgsl` raises its
//! waves with it, and the spray is blown along by it.
//!
//! The gusts are a function of the shader's clock (`Time::elapsed_secs_wrapped`), which
//! `water.wgsl` and `particles.wgsl` work out for themselves, so that nothing need be sent
//! to the graphics card again every frame. When the clock goes back to 0, once an hour, the
//! wind changes at once, a little.
//!
//! The wind is the game's own. A Monster Truck Madness 2 track does not say it has any.

use bevy::prelude::*;

use super::shore::{SwellWave, swell_waves};

/// Which way the wind blows, on the ground plane (world X and Z). Across most courses at
/// some point, and the same on every track.
const WIND_TOWARDS: Vec2 = Vec2::new(0.8, -0.6);
/// How hard it blows, in m/s: the steady part, and how far the gusts take it either side
/// of that. A fresh breeze, which ruffles water and sets whitecaps going in its gusts.
const WIND_SPEED: f32 = 6.0;
const GUSTINESS: f32 = 3.5;
/// How long the gusts take to come and go, in seconds. Three periods that don't line up,
/// so that they never settle into a beat.
const GUST_PERIODS: [f32; 3] = [11.0, 4.7, 2.3];
/// How much of `GUSTINESS` each of those swings is: the slow swing most of it, and the
/// quick ones ripple on top.
const GUST_WEIGHTS: [f32; 3] = [0.55, 0.3, 0.15];

/// The wind, which the `particles` slice carries the spray on. Only in an app that can draw.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    /// Which way it blows, of length 1.
    pub(super) towards: Vec2,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            towards: WIND_TOWARDS.normalize(),
        }
    }
}

impl Wind {
    /// How long each of the gusts' three swings takes, in seconds, and how much of `gusts`
    /// each is.
    pub const GUST_PERIODS: [f32; 3] = GUST_PERIODS;
    pub const GUST_WEIGHTS: [f32; 3] = GUST_WEIGHTS;

    /// The swell that this wind raises, as the water's surface is drawn with it.
    pub fn swell(&self) -> [SwellWave; 3] {
        swell_waves(self.towards)
    }

    /// How it blows between gusts, on the ground plane, in m/s.
    pub fn steady(&self) -> Vec3 {
        self.along() * WIND_SPEED
    }

    /// How far a full gust takes it from `steady`, in m/s.
    pub fn gusts(&self) -> Vec3 {
        self.along() * GUSTINESS
    }

    /// How it blows when the shader's clock reads `time`, in m/s: `steady`, and `gusts`
    /// times a mix of three swings of the clock. As `water.wgsl` has it.
    pub fn at(&self, time: f32) -> Vec3 {
        let tau = std::f32::consts::TAU;
        let swing: f32 = (0..3)
            .map(|index| GUST_WEIGHTS[index] * (tau * time / GUST_PERIODS[index]).sin())
            .sum();
        self.steady() + self.gusts() * swing
    }

    fn along(&self) -> Vec3 {
        Vec3::new(self.towards.x, 0.0, self.towards.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How hard the wind blows when the shader's clock reads `time`, in m/s.
    fn wind_speed(time: f32) -> f32 {
        Wind::default().at(time).length()
    }

    #[test]
    fn the_wind_gusts_but_never_stops_or_blows_a_gale() {
        let speeds: Vec<f32> = (0..6000)
            .map(|tenth| wind_speed(tenth as f32 / 10.0))
            .collect();
        let least = speeds.iter().copied().fold(f32::INFINITY, f32::min);
        let most = speeds.iter().copied().fold(0.0, f32::max);
        assert!(least > 1.0, "{least}");
        assert!(most < WIND_SPEED + GUSTINESS + 1e-3, "{most}");
        // It really does gust.
        assert!(most - least > GUSTINESS, "{least} to {most}");
    }

    #[test]
    fn the_wind_changes_smoothly() {
        for tenth in 0..6000 {
            let time = tenth as f32 / 10.0;
            let change = (wind_speed(time + 0.1) - wind_speed(time)).abs();
            assert!(
                change < 0.5,
                "{change} m/s in a tenth of a second at {time} s"
            );
        }
    }

    #[test]
    fn the_wind_blows_one_way() {
        let wind = Wind::default();
        assert!((wind.towards.length() - 1.0).abs() < 1e-6);
        let along = Vec3::new(wind.towards.x, 0.0, wind.towards.y);
        for time in [3.0, 40.0] {
            let air = wind.at(time);
            assert!(air.normalize().distance(along) < 1e-5, "{air}");
        }
    }

    #[test]
    fn the_water_gusts_as_the_spray_does() {
        let shader = include_str!("../shaders/water.wgsl");
        assert!(shader.contains(&format!("const WIND_SPEED: f32 = {WIND_SPEED:?};")));
        assert!(shader.contains(&format!("const GUSTINESS: f32 = {GUSTINESS:?};")));
        let [slow, middle, quick] = GUST_PERIODS;
        assert!(shader.contains(&format!(
            "const GUST_PERIODS: vec3<f32> = vec3({slow:?}, {middle:?}, {quick:?});"
        )));
        let [slow, middle, quick] = GUST_WEIGHTS;
        assert!(shader.contains(&format!(
            "{slow:?} * slow + {middle:?} * middle + {quick:?} * quick"
        )));
    }
}
