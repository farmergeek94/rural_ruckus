//! The rings spreading over the water from where trucks disturb it, as the CPU keeps
//! count of them.
//!
//! The rings themselves are in the field of heights (`field`), which spreads them, and
//! which nothing reads back. This list is the CPU's own account of them: where each
//! started, when, and how strong it was, so that `shore` can tell when one comes to the
//! water's edge, from how fast the field spreads a ring and about how it fades. Only the
//! newest `MAX_RIPPLES` are kept, and a ring is forgotten once it has faded.

use bevy::prelude::*;

/// How many ripples are kept at once.
pub(super) const MAX_RIPPLES: usize = 48;
/// How long a ripple lasts, in seconds. It has faded to nothing by then.
const RIPPLE_LIFE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Ripple {
    /// Where it started, on the ground plane (world X and Z), in metres.
    pub(super) center: Vec2,
    /// When it started, as `Time::elapsed_secs`.
    pub(super) born: f32,
    /// How high its first wave is, in metres.
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
}

/// Forgets the ripples that have faded.
pub(super) fn expire_ripples(time: Res<Time>, mut ripples: ResMut<Ripples>) {
    ripples.expire(time.elapsed_secs());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ripple(born: f32) -> Ripple {
        Ripple {
            center: Vec2::new(born, 0.0),
            born,
            strength: 0.1,
        }
    }

    #[test]
    fn a_full_list_drops_its_oldest() {
        let mut ripples = Ripples::default();
        for born in 0..MAX_RIPPLES + 3 {
            ripples.add(ripple(born as f32));
        }
        assert_eq!(ripples.iter().count(), MAX_RIPPLES);
        assert_eq!(ripples.iter().next().unwrap().center.x, 3.0);
    }

    #[test]
    fn faded_ripples_are_forgotten() {
        let mut ripples = Ripples::default();
        ripples.add(ripple(0.0));
        ripples.add(ripple(3.0));
        ripples.expire(RIPPLE_LIFE + 1.0);
        assert_eq!(ripples.iter().count(), 1);
    }
}
