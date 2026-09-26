//! The rings spreading over the water, as a short list the shader draws from.
//!
//! A ripple is where it started, when, and how strong it was. The shader works out the
//! ring from its age, on its own clock: how far it has spread and how much it has faded.
//! So it is given when each ripple started rather than how old it is, and what it is given
//! changes only when a ripple starts or is forgotten, not every frame. Even then only the
//! list is written, into a buffer of its own that both of the water's materials read, so
//! that neither material is sent to the graphics card again. Only the newest `MAX_RIPPLES`
//! are kept, since the shader looks at every one for every pixel of water, and an old ring
//! is faint by the time a new one takes its place.

use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;

/// How many ripples the shader is given. It must be the length of the array in
/// `water.wgsl`, which a test checks. More costs every pixel of water more.
pub(super) const MAX_RIPPLES: usize = 48;
/// How long a ripple lasts, in seconds. The shader has faded it to nothing by then.
const RIPPLE_LIFE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Ripple {
    /// Where it started, on the ground plane (world X and Z), in metres.
    pub(super) center: Vec2,
    /// When it started, as `Time::elapsed_secs`.
    pub(super) born: f32,
    /// When it started on the shader's clock, `Time::elapsed_secs_wrapped`. Kept apart
    /// from `born`, rather than worked out from it each frame, so that it is the same
    /// number every frame.
    pub(super) born_on_clock: f32,
    /// How high its first wave is, in metres. It only tilts the surface, so this sets how
    /// much the ring catches the light, and how much foam it has.
    pub(super) strength: f32,
}

/// The ripples on the water now, oldest first.
#[derive(Resource, Default, Debug)]
pub(super) struct Ripples(Vec<Ripple>);

impl Ripples {
    /// Adds a ripple, dropping the oldest if there are too many.
    pub(super) fn add(&mut self, ripple: Ripple) {
        if self.0.len() == MAX_RIPPLES {
            self.0.remove(0);
        }
        self.0.push(ripple);
    }

    /// Forgets the ripples that have faded out by `now`.
    pub(super) fn expire(&mut self, now: f32) {
        self.0.retain(|ripple| now - ripple.born < RIPPLE_LIFE);
    }

    /// The ripples, oldest first.
    pub(super) fn iter(&self) -> impl Iterator<Item = &Ripple> {
        self.0.iter()
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    /// What the shader is given when its clock reads `clock`, which goes back to 0 every
    /// `wrap_period` seconds (`Time::wrap_period`). A ripple that started before the clock
    /// last went back is given as starting that much earlier, so that its age still comes
    /// out right.
    pub(super) fn for_shader(&self, clock: f32, wrap_period: f32) -> ShaderRipples {
        let mut sent = ShaderRipples::default();
        for (slot, ripple) in sent.ripples.iter_mut().zip(&self.0) {
            let born = if ripple.born_on_clock > clock {
                ripple.born_on_clock - wrap_period
            } else {
                ripple.born_on_clock
            };
            *slot = Vec4::new(ripple.center.x, ripple.center.y, born, ripple.strength);
        }
        sent.count = self.0.len() as u32;
        sent
    }
}

/// The ripples as `water.wgsl` reads them.
#[derive(ShaderType, Clone, Debug, PartialEq)]
pub(super) struct ShaderRipples {
    /// Per ripple: where it started (X, Z), when on the shader's clock, in seconds, and its
    /// strength.
    pub(super) ripples: [Vec4; MAX_RIPPLES],
    /// How many of `ripples` are in use, from the first.
    pub(super) count: u32,
}

impl Default for ShaderRipples {
    fn default() -> Self {
        Self {
            ripples: [Vec4::ZERO; MAX_RIPPLES],
            count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ripple(born: f32) -> Ripple {
        Ripple {
            center: Vec2::new(born, 0.0),
            born,
            born_on_clock: born,
            strength: 0.1,
        }
    }

    #[test]
    fn the_shader_is_given_when_each_ripple_started() {
        let mut ripples = Ripples::default();
        ripples.add(ripple(1.0));
        ripples.add(ripple(2.5));
        let sent = ripples.for_shader(3.0, 3600.0);
        assert_eq!(sent.count, 2);
        assert_eq!(sent.ripples[0], Vec4::new(1.0, 0.0, 1.0, 0.1));
        assert_eq!(sent.ripples[1], Vec4::new(2.5, 0.0, 2.5, 0.1));
        assert_eq!(sent.ripples[2], Vec4::ZERO);
        // The same later, so that the list need not be sent again.
        assert_eq!(ripples.for_shader(3.5, 3600.0), sent);
    }

    #[test]
    fn a_ripple_from_before_the_clock_went_back_keeps_its_age() {
        let mut ripples = Ripples::default();
        ripples.add(ripple(3599.0));
        // Two seconds on, the clock reads 1.
        let born = ripples.for_shader(1.0, 3600.0).ripples[0].z;
        assert_eq!(1.0 - born, 2.0);
    }

    #[test]
    fn a_full_list_drops_its_oldest() {
        let mut ripples = Ripples::default();
        for born in 0..MAX_RIPPLES + 3 {
            ripples.add(ripple(born as f32));
        }
        let sent = ripples.for_shader(100.0, 3600.0);
        assert_eq!(sent.count as usize, MAX_RIPPLES);
        assert_eq!(sent.ripples[0].x, 3.0);
    }

    #[test]
    fn faded_ripples_are_forgotten() {
        let mut ripples = Ripples::default();
        ripples.add(ripple(0.0));
        ripples.add(ripple(3.0));
        ripples.expire(RIPPLE_LIFE + 1.0);
        assert_eq!(ripples.for_shader(5.0, 3600.0).count, 1);
    }

    #[test]
    fn the_shader_holds_as_many_ripples_as_are_sent() {
        let shader = include_str!("water.wgsl");
        assert!(shader.contains(&format!("array<vec4<f32>, {MAX_RIPPLES}>")));
    }
}
